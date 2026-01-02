import 'package:sqlite3/sqlite3.dart';
import 'dart:io';
import 'package:flutter/foundation.dart';
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
    assert(() {
      debugPrint('getAllSymbols: start limit=$limit');
      return true;
    }());
    // Run the heavy DB query off the main isolate to avoid blocking the UI
    final rows = await compute(
      _fetchSymbols,
      {'dbPath': dbPath, 'limit': limit},
    );
    assert(() {
      debugPrint('getAllSymbols: rows=${rows.length}');
      return true;
    }());

    return rows
        .map((row) => Symbol(
              id: row['id'] as int,
              name: row['name'] as String,
              kind: row['kind'] as String,
              usr: row['usr'] as String?,
              filePath: row['file_path'] as String,
              isDefinition: (row['is_definition'] as int) == 1,
            ))
        .toList();
  }

  // Загрузка всех связей
  Future<List<Edge>> getAllEdges({int? limit}) async {
    assert(() {
      debugPrint('getAllEdges: start limit=$limit');
      return true;
    }());
    // Run edge query in background isolate to avoid blocking UI
    final rows = await compute(
      _fetchEdges,
      {'dbPath': dbPath, 'limit': limit},
    );
    assert(() {
      debugPrint('getAllEdges: rows=${rows.length}');
      return true;
    }());

    return rows
        .map((row) => Edge(
              id: row['id'] as int,
              fromSymbolId: row['from_sym'] as int,
              toSymbolId: row['to_sym'] as int,
              kind: row['kind'] as String,
              fromModuleId: row['from_module'] as int?,
              toModuleId: row['to_module'] as int?,
            ))
        .toList();
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
        .map<Map<String, dynamic>>((row) => {
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
        .map<Map<String, dynamic>>((row) => {
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

// Background isolate helper: fetch symbols
List<Map<String, Object?>> _fetchSymbols(Map<String, dynamic> args) {
  final dbPath = args['dbPath'] as String;
  final limit = args['limit'] as int?;
  assert(() {
    debugPrint('fetchSymbols isolate: start limit=$limit db=$dbPath');
    return true;
  }());
  final db = sqlite3.open(dbPath, mode: OpenMode.readOnly);
  try {
    final query = '''
      SELECT s.id, s.name, s.kind, s.usr, s.is_definition, f.path as file_path
      FROM symbols s
      JOIN files f ON s.file_id = f.id
      ${limit != null ? 'LIMIT $limit' : ''}
    ''';
    final stmt = db.prepare(query);
    final rows = stmt.select();
    final out = <Map<String, Object?>>[];

    for (final row in rows) {
      out.add({
        'id': row[0] as int,
        'name': row[1] as String,
        'kind': row[2] as String,
        'usr': row[3] as String?,
        'is_definition': row[4] as int,
        'file_path': row[5] as String,
      });
    }

    stmt.dispose();
    assert(() {
      debugPrint('fetchSymbols isolate: returning ${out.length} rows');
      return true;
    }());
    return out;
  } finally {
    db.dispose();
  }
}

// Background isolate helper: fetch edges
List<Map<String, Object?>> _fetchEdges(Map<String, dynamic> args) {
  final dbPath = args['dbPath'] as String;
  final limit = args['limit'] as int?;
  assert(() {
    debugPrint('fetchEdges isolate: start limit=$limit db=$dbPath');
    return true;
  }());
  final db = sqlite3.open(dbPath, mode: OpenMode.readOnly);
  try {
    final query = '''
      SELECT id, from_sym, to_sym, kind, from_module, to_module
      FROM edges
      WHERE from_sym IS NOT NULL AND to_sym IS NOT NULL
      ${limit != null ? 'LIMIT $limit' : ''}
    ''';

    final stmt = db.prepare(query);
    final rows = stmt.select();
    final out = <Map<String, Object?>>[];

    for (final row in rows) {
      out.add({
        'id': row[0] as int,
        'from_sym': row[1] as int,
        'to_sym': row[2] as int,
        'kind': row[3] as String,
        'from_module': row[4] as int?,
        'to_module': row[5] as int?,
      });
    }

    stmt.dispose();
    assert(() {
      debugPrint('fetchEdges isolate: returning ${out.length} rows');
      return true;
    }());
    return out;
  } finally {
    db.dispose();
  }
}
