import 'package:flutter/material.dart';
import 'dart:io';
import 'dart:math' as math;
import 'package:graphview/GraphView.dart' as graphview;
import '../models/symbol.dart';
import '../models/edge.dart';
import '../services/database_service.dart';
import 'legend_widget.dart';

class GraphViewerPage extends StatefulWidget {
  final String dbPath;

  const GraphViewerPage({super.key, required this.dbPath});

  @override
  State<GraphViewerPage> createState() => _GraphViewerPageState();
}

class _GraphViewerPageState extends State<GraphViewerPage> {
  late DatabaseService _dbService;
  List<Symbol> _symbols = [];
  List<Edge> _edges = [];
  bool _isLoading = true;
  String? _error;
  graphview.Graph? _graph;
  graphview.Algorithm? _algorithm;
  int _maxNodes = 100; // Ограничение для производительности
  double _loadProgress = 0.0;
  int _loadTotal = 0;
  final int _batchSize = 100;

  // Выбранный символ для фокуса
  Symbol? _selectedSymbol;
  Set<int> _visibleSymbolIds = {};

  // Node widget keys and overlay state
  final Map<int, GlobalKey> _nodeKeys = {};
  final GlobalKey _graphStackKey = GlobalKey();
  List<_EdgeHitbox> _edgeHitboxes = [];
  final TransformationController _transformationController =
      TransformationController();
  // Controller for GraphView's built-in camera helpers
  final graphview.GraphViewController _graphController =
      graphview.GraphViewController();
  int? _hoveredNodeId;
  int? _hoveredEdgeId;

  // Keep a small debug reference to avoid analyzer "unused" errors when parts are temporarily disabled
  void _touchUnusedFieldsForAnalyzer() {
    assert(() {
      debugPrint(
          '_touch: sel=${_selectedSymbol?.id} visible=${_visibleSymbolIds.length} edges=${_edgeHitboxes.length} hn=$_hoveredNodeId he=$_hoveredEdgeId');
      return true;
    }());
  }

  @override
  void initState() {
    super.initState();
    _loadGraph();
  }

  Future<void> _loadGraph() async {
    try {
      setState(() {
        _isLoading = true;
        _error = null;
        _loadProgress = 0.0;
        _loadTotal = 0;
      });

      _dbService = DatabaseService(widget.dbPath);
      try {
        await _dbService.open();
      } catch (e) {
        setState(() {
          _error = 'Не удалось открыть базу: $e';
          _isLoading = false;
        });
        return;
      }

      debugPrint('loadGraph: starting DB fetch (limit=$_maxNodes)');
      // Загружаем символы и связи (в фоне)
      final symbols = await _dbService.getAllSymbols(limit: _maxNodes);
      final edges = await _dbService.getAllEdges(limit: _maxNodes * 2);
      debugPrint(
          'loadGraph: fetched symbols=${symbols.length} edges=${edges.length}');

      // Фильтруем связи, оставляя только те, где оба символа есть в загруженном списке
      final symbolIds = symbols.map((s) => s.id).toSet();
      final filteredEdges = edges
          .where((e) =>
              symbolIds.contains(e.fromSymbolId) &&
              symbolIds.contains(e.toSymbolId))
          .toList();

      // Постепенно собираем граф по чанкам, чтобы не блокировать UI
      final nodeMap = <int, graphview.Node>{};
      final graph = graphview.Graph();

      setState(() {
        _symbols = [];
        _edges = [];
        _graph = graph;
        _visibleSymbolIds = {};
        _loadTotal = symbols.isNotEmpty ? symbols.length : 1;
        _loadProgress = 0.0;
      });

      final batch = _batchSize;
      for (int offset = 0; offset < symbols.length; offset += batch) {
        final chunk = symbols.skip(offset).take(batch).toList();
        for (final symbol in chunk) {
          final node = graphview.Node.Id(symbol.id);
          nodeMap[symbol.id] = node;
          graph.addNode(node);
          _symbols.add(symbol);
        }

        setState(() {
          _graph = graph;
          _symbols = List.from(_symbols);
          _visibleSymbolIds = _symbols.map((s) => s.id).toSet();
          _loadProgress = _symbols.length / _loadTotal;
        });

        debugPrint('loadGraph: added nodes ${_symbols.length}/$_loadTotal');

        // Yield to UI
        await Future.delayed(const Duration(milliseconds: 30));
        _scheduleNodePositionsUpdate();
      }

      // Добавляем ребра тоже чанками
      for (int offset = 0; offset < filteredEdges.length; offset += batch) {
        final chunk = filteredEdges.skip(offset).take(batch).toList();
        for (final edge in chunk) {
          final fromNode = nodeMap[edge.fromSymbolId];
          final toNode = nodeMap[edge.toSymbolId];
          if (fromNode != null && toNode != null) {
            graph.addEdge(fromNode, toNode);
          }
        }

        setState(() {
          _graph = graph;
        });

        debugPrint(
            'loadGraph: added edges ${math.min(offset + batch, filteredEdges.length)}/${filteredEdges.length}');

        // Yield to UI
        await Future.delayed(const Duration(milliseconds: 20));
        _scheduleNodePositionsUpdate();
      }

      setState(() {
        _edges = filteredEdges;
        _loadProgress = 1.0;
        // Пока layout не применился — показываем индикатор
        _isLoading = true;
      });

      // Рассчитываем подходящее число итераций в зависимости от размера графа
      final iterations = _calculateIterations(symbols.length);
      debugPrint('loadGraph: setting layout iterations=$iterations');

      // Небольшая задержка, чтобы дать фрейму отрисоваться и не блокировать UI немедленным тяжелым layout
      Future.delayed(const Duration(milliseconds: 50), () {
        try {
          setState(() {
            // Используем SugiyamaAlgorithm (слоистый, ориентированный на минимизацию пересечений)
            final builder = graphview.BuchheimWalkerConfiguration();
            builder
              ..siblingSeparation = 20
              ..levelSeparation = 40
              ..subtreeSeparation = 30
              ..orientation = graphview.BuchheimWalkerConfiguration.ORIENTATION_LEFT_RIGHT;

            _algorithm = graphview.BuchheimWalkerAlgorithm(builder, graphview.TreeEdgeRenderer(builder));
            
            // Сразу убрать ин��икатор загрузки, чтобы избежать зависаний
            _isLoading = false;
          });
        } catch (e, st) {
          debugPrint('Failed to set algorithm: $e\n$st');
          if (mounted) {
            setState(() => _isLoading = false);
          }
        }

        // После рендера обновим позиции узлов и hitbox'ы для ребер и автоматически впишем граф в вид
        _scheduleNodePositionsUpdate(fit: true);
      });
    } catch (e) {
      setState(() {
        _error = e.toString();
        _isLoading = false;
      });
    }
  }

  // ignore: unused_element
  graphview.Graph _buildGraph() {
    final graph = graphview.Graph();
    final nodeMap = <int, graphview.Node>{};

    // Создаем узлы
    for (var symbol in _symbols) {
      final node = graphview.Node.Id(symbol.id);
      nodeMap[symbol.id] = node;
      graph.addNode(node);
    }

    // Создаем связи
    for (var edge in _edges) {
      final fromNode = nodeMap[edge.fromSymbolId];
      final toNode = nodeMap[edge.toSymbolId];
      if (fromNode != null && toNode != null) {
        graph.addEdge(fromNode, toNode);
      }
    }

    return graph;
  }

  // ignore: unused_element
  void _onNodeTap(graphview.Node node) {
    final symbolId = node.key!.value as int;
    final symbol = _symbols.firstWhere((s) => s.id == symbolId);

    setState(() {
      _selectedSymbol = symbol;
    });

    // Показываем информацию о символе
    _showSymbolInfo(symbol);
  }

  Future<void> _showSymbolInfo(Symbol symbol) async {
    // Read file and prepare snippet with line numbers
    List<String> snippetLines = [];
    String infoMessage = '';
    try {
      final file = File(symbol.filePath);
      if (!await file.exists()) {
        infoMessage = 'Файл не найден: ${symbol.filePath}';
      } else {
        final lines = await file.readAsLines();
        final regex = RegExp('\\b${RegExp.escape(symbol.name)}\\b');
        int matchIndex = -1;
        for (int i = 0; i < lines.length; i++) {
          if (regex.hasMatch(lines[i])) {
            matchIndex = i;
            break;
          }
        }
        if (matchIndex == -1) {
          infoMessage = 'Символ не найден в файле; показываю начало файла';
          final end = lines.length < 10 ? lines.length : 10;
          for (int i = 0; i < end; i++) {
            final prefix = (i + 1).toString().padLeft(4);
            snippetLines.add('$prefix: ${lines[i]}');
          }
        } else {
          infoMessage = 'Строка ${matchIndex + 1}';
          final start = (matchIndex - 3) < 0 ? 0 : (matchIndex - 3);
          final end = (matchIndex + 3) >= lines.length
              ? (lines.length - 1)
              : (matchIndex + 3);
          for (int i = start; i <= end; i++) {
            final prefix = (i + 1).toString().padLeft(4);
            snippetLines.add('$prefix: ${lines[i]}');
          }
        }
      }
    } catch (e) {
      infoMessage = 'Ошибка чтения файла: $e';
    }

    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(symbol.name),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Тип: ${symbol.kind}'),
              const SizedBox(height: 8),
              Text('Файл: ${symbol.filePath}'),
              const SizedBox(height: 8),
              Text(infoMessage),
              const SizedBox(height: 8),
              Container(
                width: double.maxFinite,
                decoration: BoxDecoration(
                  color: Colors.black12,
                  borderRadius: BorderRadius.circular(4),
                ),
                padding: const EdgeInsets.all(8),
                child: snippetLines.isNotEmpty
                    ? SelectableText(
                        snippetLines.join('\n'),
                        style: const TextStyle(
                            fontFamily: 'monospace', fontSize: 12),
                      )
                    : const Text('(Нет данных для отображения)'),
              ),
              const SizedBox(height: 8),
              Text('Определение: ${symbol.isDefinition ? "Да" : "Нет"}'),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Закрыть'),
          ),
          TextButton(
            onPressed: () {
              _openFile(symbol.filePath);
              Navigator.pop(context);
            },
            child: const Text('Открыть файл'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    // Touch temporarily-unused fields in debug mode to avoid analyzer warnings while refactoring
    assert(() {
      _touchUnusedFieldsForAnalyzer();
      return true;
    }());

    if (_isLoading) {
      final pct =
          _loadTotal > 0 ? (_loadProgress * 100).toStringAsFixed(0) : '0';
      return Scaffold(
        appBar: AppBar(title: Text('Загрузка графа... ($pct%)')),
        body: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const CircularProgressIndicator(),
              const SizedBox(height: 12),
              Text('${_symbols.length}/$_loadTotal узлов загружено'),
              const SizedBox(height: 8),
              const Text('Пожалуйста подождите...'),
            ],
          ),
        ),
      );
    }

    if (_error != null) {
      return Scaffold(
        appBar: AppBar(title: const Text('Ошибка')),
        body: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text('Ошибка: $_error'),
              const SizedBox(height: 16),
              ElevatedButton(
                onPressed: () => Navigator.pop(context),
                child: const Text('Назад'),
              ),
            ],
          ),
        ),
      );
    }

    if (_graph == null || _algorithm == null) {
      return const Scaffold(
        body: Center(child: Text('Граф не загружен')),
      );
    }

    return Scaffold(
      appBar: AppBar(
        title: Text(
            'Граф символов (${_symbols.length} узлов, ${_edges.length} связей)'),
        actions: [
          IconButton(
            icon: const Icon(Icons.info_outline),
            onPressed: () => _showStatistics(),
            tooltip: 'Статистика',
          ),
          IconButton(
            icon: const Icon(Icons.fit_screen),
            onPressed: () => _updateNodePositions(fit: true),
            tooltip: 'Вписать в экран',
          ),
          IconButton(
            icon: const Icon(Icons.filter_list),
            onPressed: () => _showFilterDialog(),
            tooltip: 'Фильтры',
          ),
        ],
      ),
      body: Row(
        children: [
          // Легенда
          SizedBox(
            width: 200,
            child: LegendWidget(
              symbols: _symbols,
              edges: _edges,
            ),
          ),
          // Граф
          Expanded(
            child: InteractiveViewer(
              constrained: true,
              transformationController: _transformationController,
              boundaryMargin: const EdgeInsets.all(2000),
              minScale: 0.1,
              maxScale: 4.0,
              child: SizedBox.expand(
                child: Stack(
                  key: _graphStackKey,
                  children: [
                  // The interactive graph rendering using GraphView
                  Positioned.fill(
                    child: (_graph != null && _algorithm != null)
                        ? graphview.GraphView.builder(
                            graph: _graph!,
                            algorithm: _algorithm!,
                            // Custom rendering for each node
                            builder: (graphview.Node node) {
                              final id = node.key!.value as int;
                              final symbol = _symbols.firstWhere((s) => s.id == id,
                                  orElse: () => Symbol(
                                      id: id,
                                      name: 'n$id',
                                      kind: '',
                                      filePath: '',
                                      isDefinition: false));
                              final key = _nodeKeys.putIfAbsent(id, () => GlobalKey());
                              return GestureDetector(
                              key: key,
                                onTap: () => _onNodeTap(node),
                                child: Container(
                                  padding: const EdgeInsets.symmetric(
                                      horizontal: 8, vertical: 6),
                                  decoration: BoxDecoration(
                                    color: _selectedSymbol?.id == id
                                        ? Colors.lightBlueAccent
                                        : Colors.white,
                                    border: Border.all(color: Colors.black12),
                                    borderRadius: BorderRadius.circular(6),
                                    boxShadow: const [
                                      BoxShadow(color: Colors.black12, blurRadius: 2)
                                    ],
                                  ),
                                  constraints: const BoxConstraints(minWidth: 32),
                                  child: Text(
                                    symbol.name,
                                    overflow: TextOverflow.ellipsis,
                                    style: const TextStyle(fontSize: 12),
                                  ),
                                ),
                              );
                            },
                          )
                        : const SizedBox.shrink(),
                  ),

                  // Transparent overlay: detects taps on edges and paints simple edge lines
                  Positioned.fill(
                    child: EdgeOverlay(
                      hitboxes: _edgeHitboxes,
                      onEdgeTap: _onEdgeTap,
                    ),
                  ),
                ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  int _calculateIterations(int nodeCount) {
    if (nodeCount <= 100) return 1000;
    if (nodeCount <= 300) return 600;
    if (nodeCount <= 600) return 300;
    if (nodeCount <= 1200) return 150;
    return 80; // for very large graphs keep it small
  }

  // Позволяет выбирать альтернативный алгоритм через меню, при необходимости
  void _useCrossingMinimizingLayout() {
    final builder = graphview.BuchheimWalkerConfiguration();
    builder
      ..siblingSeparation = 20
      ..levelSeparation = 40
      ..subtreeSeparation = 30
      ..orientation = graphview.BuchheimWalkerConfiguration.ORIENTATION_LEFT_RIGHT;
    setState(() {
      _algorithm = graphview.BuchheimWalkerAlgorithm(builder, TreeEdgeRenderer(builder));
    });
    _scheduleNodePositionsUpdate(fit: true);
  }

  void _scheduleNodePositionsUpdate({bool fit = false}) {
    if (!mounted) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _updateNodePositions(fit: fit);
    });
  }

  void _showStatistics() async {
    debugPrint('showStatistics: fetching stats');
    if (!mounted) return;
    Map<String, dynamic> stats;
    try {
      stats = await _dbService.getStatistics();
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Не удалось получить статис��ику: $e')),
      );
      return;
    }
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Статистика'),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Символов: ${stats['symbolCount']}'),
              Text('Связей: ${stats['edgeCount']}'),
              Text('Файлов: ${stats['fileCount']}'),
              const SizedBox(height: 16),
              const Text('Типы символов:',
                  style: TextStyle(fontWeight: FontWeight.bold)),
              ...(stats['symbolKinds'] as List).map((row) => Padding(
                    padding: const EdgeInsets.only(left: 16),
                    child: Text('  ${row['kind']}: ${row['count']}'),
                  )),
              const SizedBox(height: 16),
              const Text('Типы связей:',
                  style: TextStyle(fontWeight: FontWeight.bold)),
              ...(stats['edgeKinds'] as List).map((row) => Padding(
                    padding: const EdgeInsets.only(left: 16),
                    child: Text('  ${row['kind']}: ${row['count']}'),
                  )),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Закрыть'),
          ),
        ],
      ),
    );
  }

  void _showFilterDialog() {
    final controller = TextEditingController(text: _maxNodes.toString());
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Фильтры'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: controller,
              decoration: const InputDecoration(
                labelText: 'Максимум узлов',
                hintText: '100',
              ),
              keyboardType: TextInputType.number,
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Отмена'),
          ),
          TextButton(
            onPressed: () {
              final limit = int.tryParse(controller.text);
              if (limit != null && limit > 0) {
                setState(() {
                  _maxNodes = limit;
                });
                _loadGraph();
                Navigator.pop(context);
              }
            },
            child: const Text('Применить'),
          ),
        ],
      ),
    );
  }

  Future<void> _openFile(String path) async {
    try {
      if (Platform.isWindows) {
        await Process.run('cmd', ['/C', 'start', '', path]);
      } else if (Platform.isMacOS) {
        await Process.run('open', [path]);
      } else if (Platform.isLinux) {
        await Process.run('xdg-open', [path]);
      }
    } catch (_) {
      // ignore errors
    }
  }

  // Обновляем позиции виджетов узлов и генерируем hitbox'ы для ребер
  void _updateNodePositions({bool fit = false, int attempts = 5}) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final stackBox =
          _graphStackKey.currentContext?.findRenderObject() as RenderBox?;
      if (stackBox == null) return;

      final positions = <int, Offset>{};
      _nodeKeys.forEach((id, key) {
        final rb = key.currentContext?.findRenderObject() as RenderBox?;
        if (rb == null) return;
        final globalCenter = rb.localToGlobal(rb.size.center(Offset.zero));
        final local = stackBox.globalToLocal(globalCenter);
        positions[id] = local;
      });

      // If positions are empty or too few, GraphView may not have finished layout yet — retry a few times
      final posCount = positions.length;
      final expected = _nodeKeys.length;
      if (posCount == 0 || (fit && expected > 0 && posCount < (expected * 0.5))) {
        if (attempts > 0) {
          WidgetsBinding.instance.addPostFrameCallback(
              (_) => _updateNodePositions(fit: fit, attempts: attempts - 1));
          return;
        } else {
          debugPrint('updateNodePositions: insufficient node positions after retries pos=$posCount exp=$expected');
        }
      }

      final newHitboxes = <_EdgeHitbox>[];
      for (final edge in _edges) {
        final p1 = positions[edge.fromSymbolId];
        final p2 = positions[edge.toSymbolId];
        if (p1 != null && p2 != null) {
          newHitboxes.add(_EdgeHitbox(edge: edge, p1: p1, p2: p2));
        }
      }

      setState(() {
        _edgeHitboxes = newHitboxes;
      });

      // По возможности подогнать масштаб, чтобы граф был виден полностью — только при явном запросе
      if (fit) _fitGraphToView();
    });
  }

  // Подгон масштаба так, чтобы граф полностью помещался в видимую область
  void _fitGraphToView() {
    try {
      final stackBox =
          _graphStackKey.currentContext?.findRenderObject() as RenderBox?;
      if (stackBox == null) return;

      // Reset transform to avoid compounding previous pan/zoom
      _transformationController.value = Matrix4.identity();

      double minX = double.infinity,
          minY = double.infinity,
          maxX = -double.infinity,
          maxY = -double.infinity;
      for (final entry in _nodeKeys.entries) {
        final rb = entry.value.currentContext?.findRenderObject() as RenderBox?;
        if (rb == null) continue;
        final globalCenter = rb.localToGlobal(rb.size.center(Offset.zero));
        final local = stackBox.globalToLocal(globalCenter);
        minX = math.min(minX, local.dx);
        minY = math.min(minY, local.dy);
        maxX = math.max(maxX, local.dx);
        maxY = math.max(maxY, local.dy);
      }

      if (minX == double.infinity) return;

      const padding = 40.0; // padding around content
      final contentW = (maxX - minX) + padding;
      final contentH = (maxY - minY) + padding;
      final viewW = stackBox.size.width;
      final viewH = stackBox.size.height;

      if (contentW <= 0 || contentH <= 0 || viewW <= 0 || viewH <= 0) return;

      var scale = math.min(viewW / contentW, viewH / contentH) * 0.95;

      // Clamp scale to InteractiveViewer limits so we don't zoom too far out/in
      scale = scale.clamp(0.1, 4.0);
      if (scale.isNaN || scale.isInfinite) return;

      // Compute content center and align it to view center (more stable than top-left logic)
      final cx = (minX + maxX) / 2.0;
      final cy = (minY + maxY) / 2.0;
      final tx = (viewW / 2.0) - cx * scale;
      final ty = (viewH / 2.0) - cy * scale;

      final matrix = Matrix4.identity()
        ..translate(tx, ty)
        ..scale(scale);

      if (!mounted) return;
      setState(() {
        _transformationController.value = matrix;
      });
    } catch (e, st) {
      // Log and ignore — prevent crash during layout / rebuild races
      debugPrint('fitGraphToView failed: $e\n$st');
    }
  }

  // Distance from point p to segment a-b
  double _distanceToSegment(Offset p, Offset a, Offset b) {
    final ab = b - a;
    final abLen2 = ab.dx * ab.dx + ab.dy * ab.dy;
    if (abLen2 == 0) return (p - a).distance;
    final ap = p - a;
    var t = (ap.dx * ab.dx + ap.dy * ab.dy) / abLen2;
    t = t.clamp(0.0, 1.0);
    final proj = Offset(a.dx + ab.dx * t, a.dy + ab.dy * t);
    return (p - proj).distance;
  }

  // ignore: unused_element
  void _onEdgeTap(Edge edge) {
    final from = _symbols.firstWhere((s) => s.id == edge.fromSymbolId,
        orElse: () => Symbol(
            id: -1,
            name: 'Unknown',
            kind: '',
            filePath: '',
            isDefinition: false));
    final to = _symbols.firstWhere((s) => s.id == edge.toSymbolId,
        orElse: () => Symbol(
            id: -1,
            name: 'Unknown',
            kind: '',
            filePath: '',
            isDefinition: false));

    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Связь: ${edge.kind}'),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Из: ${from.name} (id=${edge.fromSymbolId})'),
              const SizedBox(height: 8),
              Text('В: ${to.name} (id=${edge.toSymbolId})'),
              const SizedBox(height: 8),
              Text('ID связи: ${edge.id}'),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Закрыть'),
          ),
        ],
      ),
    );
  }

  @override
  void dispose() {
    _transformationController.dispose();
    try {
      _dbService.close();
    } catch (_) {}
    super.dispose();
  }
}

// Примитивная структура для хранения геометрии ребра в координатах контейнера
class _EdgeHitbox {
  final Edge edge;
  final Offset p1;
  final Offset p2;

  _EdgeHitbox({required this.edge, required this.p1, required this.p2});
}

class _EdgePainter extends CustomPainter {
  final List<_EdgeHitbox> hitboxes;
  _EdgePainter(this.hitboxes);

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = Colors.grey.shade400
      ..strokeWidth = 1.0
      ..style = PaintingStyle.stroke;
    for (final hb in hitboxes) {
      canvas.drawLine(hb.p1, hb.p2, paint);
    }
  }

  @override
  bool shouldRepaint(covariant _EdgePainter old) => old.hitboxes != hitboxes;
}

class EdgeOverlay extends StatelessWidget {
  final List<_EdgeHitbox> hitboxes;
  final void Function(Edge) onEdgeTap;
  const EdgeOverlay({super.key, required this.hitboxes, required this.onEdgeTap});

  double _distanceToSegment(Offset p, Offset a, Offset b) {
    final ab = b - a;
    final abLen2 = ab.dx * ab.dx + ab.dy * ab.dy;
    if (abLen2 == 0) return (p - a).distance;
    final ap = p - a;
    var t = (ap.dx * ab.dx + ap.dy * ab.dy) / abLen2;
    t = t.clamp(0.0, 1.0);
    final proj = Offset(a.dx + ab.dx * t, a.dy + ab.dy * t);
    return (p - proj).distance;
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onTapUp: (details) {
        final local = details.localPosition;
        Edge? hit;
        double best = double.infinity;
        for (final hb in hitboxes) {
          final d = _distanceToSegment(local, hb.p1, hb.p2);
          if (d < 12.0 && d < best) {
            best = d;
            hit = hb.edge;
          }
        }
        if (hit != null) onEdgeTap(hit);
      },
      child: CustomPaint(
        painter: _EdgePainter(hitboxes),
        size: Size.infinite,
      ),
    );
  }
}
