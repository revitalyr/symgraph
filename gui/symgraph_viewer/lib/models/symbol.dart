import 'package:flutter/material.dart';

class Symbol {
  final int id;
  final String name;
  final String kind;
  final String? usr;
  final String filePath;
  final bool isDefinition;

  Symbol({
    required this.id,
    required this.name,
    required this.kind,
    this.usr,
    required this.filePath,
    required this.isDefinition,
  });

  // Цвета для разных типов символов
  static Color getColorForKind(String kind) {
    switch (kind.toLowerCase()) {
      case 'function':
      case 'functiondecl':
        return Colors.blue;
      case 'class':
      case 'struct':
      case 'classdecl':
        return Colors.green;
      case 'variable':
      case 'var':
      case 'vardecl':
        return Colors.orange;
      case 'method':
      case 'cxxmethod':
        return Colors.purple;
      case 'namespace':
      case 'namespacealias':
        return Colors.teal;
      case 'enum':
      case 'enumerator':
        return Colors.pink;
      case 'typedef':
      case 'typealias':
        return Colors.indigo;
      case 'module':
        return Colors.cyan;
      default:
        return Colors.grey;
    }
  }

  // Иконки для разных типов символов
  static IconData getIconForKind(String kind) {
    switch (kind.toLowerCase()) {
      case 'function':
      case 'functiondecl':
        return Icons.functions;
      case 'class':
      case 'struct':
      case 'classdecl':
        return Icons.class_;
      case 'variable':
      case 'var':
      case 'vardecl':
        return Icons.label;
      case 'method':
      case 'cxxmethod':
        return Icons.code;
      case 'namespace':
        return Icons.folder;
      case 'enum':
        return Icons.list;
      case 'module':
        return Icons.extension;
      default:
        return Icons.circle;
    }
  }
}

