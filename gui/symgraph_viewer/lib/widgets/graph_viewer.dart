import 'package:flutter/material.dart';
import 'dart:io';
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

  // Выбранный символ для фокуса
  Symbol? _selectedSymbol;
  Set<int> _visibleSymbolIds = {};

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
      });

      _dbService = DatabaseService(widget.dbPath);
      await _dbService.open();

      // Загружаем символы и связи
      final symbols = await _dbService.getAllSymbols(limit: _maxNodes);
      final edges = await _dbService.getAllEdges(limit: _maxNodes * 2);

      // Фильтруем связи, оставляя только те, где оба символа есть в загруженном списке
      final symbolIds = symbols.map((s) => s.id).toSet();
      final filteredEdges = edges
          .where((e) =>
              symbolIds.contains(e.fromSymbolId) &&
              symbolIds.contains(e.toSymbolId))
          .toList();

      setState(() {
        _symbols = symbols;
        _edges = filteredEdges;
        _graph = _buildGraph();
        // Используем FruchtermanReingold с конфигурацией
        _algorithm = graphview.FruchtermanReingoldAlgorithm(
          graphview.FruchtermanReingoldConfiguration(
            iterations: 1000,
          ),
        );
        _isLoading = false;
        _visibleSymbolIds = symbolIds;
      });
    } catch (e) {
      setState(() {
        _error = e.toString();
        _isLoading = false;
      });
    }
  }

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
            snippetLines.add('${(i + 1).toString().padLeft(4)}: ${lines[i]}');
          }
        } else {
          infoMessage = 'Строка ${matchIndex + 1}';
          final start = (matchIndex - 3) < 0 ? 0 : (matchIndex - 3);
          final end = (matchIndex + 3) >= lines.length
              ? (lines.length - 1)
              : (matchIndex + 3);
          for (int i = start; i <= end; i++) {
            final prefix = '${(i + 1).toString().padLeft(4)}';
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
    if (_isLoading) {
      return Scaffold(
        appBar: AppBar(title: const Text('Загрузка графа...')),
        body: const Center(child: CircularProgressIndicator()),
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
              constrained: false,
              minScale: 0.1,
              maxScale: 4.0,
              child: graphview.GraphView(
                graph: _graph!,
                algorithm: _algorithm!,
                builder: (graphview.Node node) {
                  final symbolId = node.key!.value as int;
                  final symbol = _symbols.firstWhere((s) => s.id == symbolId);
                  final color = Symbol.getColorForKind(symbol.kind);
                  final icon = Symbol.getIconForKind(symbol.kind);
                  final isSelected = _selectedSymbol?.id == symbol.id;

                  return GestureDetector(
                    onTap: () => _onNodeTap(node),
                    child: Container(
                      width: 80,
                      height: 80,
                      decoration: BoxDecoration(
                        color: color.withOpacity(0.2),
                        border: Border.all(
                          color: isSelected ? Colors.red : color,
                          width: isSelected ? 3 : 2,
                        ),
                        borderRadius: BorderRadius.circular(40),
                        boxShadow: isSelected
                            ? [
                                BoxShadow(
                                  color: Colors.red.withOpacity(0.5),
                                  blurRadius: 8,
                                  spreadRadius: 2,
                                ),
                              ]
                            : null,
                      ),
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Icon(icon, color: color, size: 24),
                          const SizedBox(height: 4),
                          Text(
                            symbol.name.length > 10
                                ? '${symbol.name.substring(0, 10)}...'
                                : symbol.name,
                            style: TextStyle(
                              fontSize: 10,
                              color: color,
                              fontWeight: FontWeight.bold,
                            ),
                            textAlign: TextAlign.center,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ],
                      ),
                    ),
                  );
                },
              ),
            ),
          ),
        ],
      ),
    );
  }

  void _showStatistics() async {
    final stats = await _dbService.getStatistics();
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

  @override
  void dispose() {
    _dbService.close();
    super.dispose();
  }
}
