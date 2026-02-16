import 'package:flutter/material.dart';

class MaterialErrorCard extends StatelessWidget {
  const MaterialErrorCard({
    super.key,
    required this.message,
    this.onDismiss,
  });

  final String message;
  final VoidCallback? onDismiss;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return Card(
      color: cs.errorContainer,
      child: ListTile(
        leading: Icon(Icons.error_outline, color: cs.onErrorContainer),
        title: Text('错误', style: TextStyle(color: cs.onErrorContainer)),
        subtitle: Text(message, style: TextStyle(color: cs.onErrorContainer)),
        trailing: onDismiss == null
            ? null
            : IconButton(
                tooltip: '关闭',
                icon: Icon(Icons.close, color: cs.onErrorContainer),
                onPressed: onDismiss,
              ),
      ),
    );
  }
}
