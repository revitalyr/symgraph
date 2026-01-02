import 'package:flutter/material.dart';
import 'dart:io';
import 'widgets/graph_viewer.dart';

void main() {
  runApp(const SymgraphViewerApp());
}

class SymgraphViewerApp extends StatelessWidget {
  const SymgraphViewerApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Symgraph Viewer',
      theme: ThemeData(
        primarySwatch: Colors.blue,
        useMaterial3: true,
      ),
      home: const DatabaseSelectorPage(),
    );
  }
}

class DatabaseSelectorPage extends StatefulWidget {
  const DatabaseSelectorPage({super.key});

  @override
  State<DatabaseSelectorPage> createState() => _DatabaseSelectorPageState();
}

class _DatabaseSelectorPageState extends State<DatabaseSelectorPage> {
  final TextEditingController _pathController = TextEditingController();
  String? _selectedPath;

  void _selectDatabase() async {
    // В реальном приложении здесь можно использовать file_picker
    final path = _pathController.text.trim();
    if (path.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Пожалуйста, введите путь к базе данных')),
      );
      return;
    }

    final file = File(path);
    if (!await file.exists()) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Файл не найден')),
      );
      return;
    }

    setState(() {
      _selectedPath = path;
    });

    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (context) => GraphViewerPage(dbPath: path),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Symgraph Viewer'),
      ),
      body: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Text(
              'Выберите базу данных Symgraph',
              style: TextStyle(fontSize: 20, fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 24),
            TextField(
              controller: _pathController,
              decoration: const InputDecoration(
                labelText: 'Путь к базе данных (.db)',
                border: OutlineInputBorder(),
                hintText: 'например: mydatabase.db',
              ),
            ),
            const SizedBox(height: 24),
            ElevatedButton(
              onPressed: _selectDatabase,
              style: ElevatedButton.styleFrom(
                padding:
                    const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
              ),
              child: const Text('Загрузить граф'),
            ),
          ],
        ),
      ),
    );
  }

  @override
  void dispose() {
    _pathController.dispose();
    super.dispose();
  }
}
