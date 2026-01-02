import 'package:flutter/material.dart';

class Edge {
  final int id;
  final int fromSymbolId;
  final int toSymbolId;
  final String kind;
  final int? fromModuleId;
  final int? toModuleId;

  Edge({
    required this.id,
    required this.fromSymbolId,
    required this.toSymbolId,
    required this.kind,
    this.fromModuleId,
    this.toModuleId,
  });

  // Цвета для разных типов связей
  static Color getColorForKind(String kind) {
    switch (kind.toLowerCase()) {
      case 'call':
        return Colors.red;
      case 'inherit':
      case 'inheritance':
        return Colors.blue;
      case 'member':
        return Colors.green;
      case 'module-import':
      case 'import':
        return Colors.purple;
      case 'reference':
        return Colors.orange;
      case 'type_ref':
        return Colors.teal;
      default:
        return Colors.grey;
    }
  }

  // Толщина линии в зависимости от типа связи
  static double getWidthForKind(String kind) {
    switch (kind.toLowerCase()) {
      case 'call':
        return 2.0;
      case 'inherit':
        return 3.0;
      case 'member':
        return 1.5;
      case 'module-import':
        return 2.5;
      default:
        return 1.0;
    }
  }

  // Стиль линии
  static PaintingStyle getStyleForKind(String kind) {
    switch (kind.toLowerCase()) {
      case 'inherit':
        return PaintingStyle.stroke;
      default:
        return PaintingStyle.fill;
    }
  }
}

