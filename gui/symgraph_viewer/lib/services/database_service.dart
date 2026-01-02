import 'package:sqlite3/sqlite3.dart';
import 'dart:io';
import '../models/symbol.dart';
import '../models/edge.dart';

class DatabaseService {
  late Database _database;
  final String dbPath;

  DatabaseService(this.dbPath);

  Future<void> open() async {
    if (!File(dbPath).existsSync()) {
      throw Exception('База данных не найдена: $dbPath');
    }

    _database = sqlite3.open(dbPath, mode: OpenMode.readOnly);
  }

  Future<void> close() async {
    _database.dispose();
  }

  // Загрузка всех символов
  Future<List<Symbol>> getAllSymbols({int? limit}) async {
    final query = '''
      SELECT s.id, s.name, s.kind, s.usr, s.is_definition, f.path as file_path
      FROM symbols s
      JOIN files f ON s.file_id = f.id
      ${limit != null ? 'LIMIT $limit' : ''}
    ''';

    final stmt = _database.prepare(query);
    final rows = stmt.select();
    final symbols = <Symbol>[];

    for (final row in rows) {
      symbols.add(Symbol(
        id: row[0] as int,
        name: row[1] as String,
        kind: row[2] as String,
        usr: row[3] as String?,
        filePath: row[5] as String,
        isDefinition: (row[4] as int) == 1,
      ));
    }

    stmt.dispose();
    return symbols;
  }

  // Загрузка всех связей
  Future<List<Edge>> getAllEdges({int? limit}) async {
    final query = '''
      SELECT id, from_sym, to_sym, kind, from_module, to_module
      FROM edges
      WHERE from_sym IS NOT NULL AND to_sym IS NOT NULL
      ${limit != null ? 'LIMIT $limit' : ''}
    ''';

    final stmt = _database.prepare(query);
    final rows = stmt.select();
    final edges = <Edge>[];

    for (final row in rows) {
      edges.add(Edge(
        id: row[0] as int,
        fromSymbolId: row[1] as int,
        toSymbolId: row[2] as int,
        kind: row[3] as String,
        fromModuleId: row[4] as int?,
        toModuleId: row[5] as int?,
      ));
    }

    stmt.dispose();
    return edges;
  }

  // Загрузка символов с фильтрацией по типу
  Future<List<Symbol>> getSymbolsByKind(String kind) async {
    const query = '''
      SELECT s.id, s.name, s.kind, s.usr, s.is_definition, f.path as file_path
      FROM symbols s
      JOIN files f ON s.file_id = f.id
      WHERE s.kind = ?
    ''';

    final stmt = _database.prepare(query);
    stmt.execute([kind]);
    final rows = stmt.select();
    final symbols = <Symbol>[];

    for (final row in rows) {
      symbols.add(Symbol(
        id: row[0] as int,
        name: row[1] as String,
        kind: row[2] as String,
        usr: row[3] as String?,
        filePath: row[5] as String,
        isDefinition: (row[4] as int) == 1,
      ));
    }

    stmt.dispose();
    return symbols;
  }

  // Получение статистики
  Future<Map<String, dynamic>> getStatistics() async {
    // Подсчет символов
    final symbolStmt = _database.prepare('SELECT COUNT(*) FROM symbols');
    final symbolCount = symbolStmt.select().first[0] as int;
    symbolStmt.dispose();

    // Подсчет связей
    final edgeStmt = _database.prepare('SELECT COUNT(*) FROM edges');
    final edgeCount = edgeStmt.select().first[0] as int;
    edgeStmt.dispose();

    // Подсчет файлов
    final fileStmt = _database.prepare('SELECT COUNT(*) FROM files');
    final fileCount = fileStmt.select().first[0] as int;
    fileStmt.dispose();

    // Статистика по типам символов
    final symbolKindsStmt = _database.prepare('''
      SELECT kind, COUNT(*) as count
      FROM symbols
      GROUP BY kind
      ORDER BY count DESC
    ''');
    final symbolKindsRows = symbolKindsStmt.select();
    final symbolKinds = symbolKindsRows
        .map((row) => {
              'kind': row[0] as String,
              'count': row[1] as int,
            })
        .toList();
    symbolKindsStmt.dispose();

    // Статистика по типам связей
    final edgeKindsStmt = _database.prepare('''
      SELECT kind, COUNT(*) as count
      FROM edges
      GROUP BY kind
      ORDER BY count DESC
    ''');
    final edgeKindsRows = edgeKindsStmt.select();
    final edgeKinds = edgeKindsRows
        .map((row) => {
              'kind': row[0] as String,
              'count': row[1] as int,
            })
        .toList();
    edgeKindsStmt.dispose();

    return {
      'symbolCount': symbolCount,
      'edgeCount': edgeCount,
      'fileCount': fileCount,
      'symbolKinds': symbolKinds,
      'edgeKinds': edgeKinds,
    };
  }
}
