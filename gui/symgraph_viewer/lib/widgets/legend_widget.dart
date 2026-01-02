import 'package:flutter/material.dart';
import '../models/symbol.dart';
import '../models/edge.dart';

class LegendWidget extends StatelessWidget {
  final List<Symbol> symbols;
  final List<Edge> edges;

  const LegendWidget({
    super.key,
    required this.symbols,
    required this.edges,
  });

  @override
  Widget build(BuildContext context) {
    // Получаем уникальные типы символов и связей
    final symbolKinds = symbols.map((s) => s.kind).toSet().toList()..sort();
    final edgeKinds = edges.map((e) => e.kind).toSet().toList()..sort();

    return Container(
      color: Colors.grey[100],
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          const Text(
            'Легенда',
            style: TextStyle(
              fontSize: 18,
              fontWeight: FontWeight.bold,
            ),
          ),
          const SizedBox(height: 16),
          const Text(
            'Типы символов:',
            style: TextStyle(
              fontSize: 14,
              fontWeight: FontWeight.bold,
            ),
          ),
          const SizedBox(height: 8),
          ...symbolKinds.map((kind) => _buildSymbolLegendItem(kind)),
          const SizedBox(height: 24),
          const Text(
            'Типы связей:',
            style: TextStyle(
              fontSize: 14,
              fontWeight: FontWeight.bold,
            ),
          ),
          const SizedBox(height: 8),
          ...edgeKinds.map((kind) => _buildEdgeLegendItem(kind)),
        ],
      ),
    );
  }

  Widget _buildSymbolLegendItem(String kind) {
    final color = Symbol.getColorForKind(kind);
    final icon = Symbol.getIconForKind(kind);

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Container(
            width: 24,
            height: 24,
            decoration: BoxDecoration(
              color: color.withOpacity(0.2),
              border: Border.all(color: color, width: 2),
              borderRadius: BorderRadius.circular(12),
            ),
            child: Icon(icon, color: color, size: 16),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              kind,
              style: const TextStyle(fontSize: 12),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildEdgeLegendItem(String kind) {
    final color = Edge.getColorForKind(kind);
    final width = Edge.getWidthForKind(kind);

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Container(
            width: 40,
            height: 2,
            color: color,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              kind,
              style: const TextStyle(fontSize: 12),
            ),
          ),
        ],
      ),
    );
  }
}

