import 'package:flutter/material.dart';

class NetworkImageView extends StatelessWidget {
  const NetworkImageView({
    super.key,
    required this.url,
    this.fit = BoxFit.cover,
    this.borderRadius = 8,
  });

  final String? url;
  final BoxFit fit;
  final double borderRadius;

  @override
  Widget build(BuildContext context) {
    final u = (url ?? '').trim();
    final placeholder = Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(borderRadius),
      ),
      child: const Center(child: Icon(Icons.image_not_supported_outlined)),
    );

    if (u.isEmpty) return placeholder;

    return ClipRRect(
      borderRadius: BorderRadius.circular(borderRadius),
      child: Image.network(
        u,
        fit: fit,
        errorBuilder: (context, _, __) => placeholder,
        loadingBuilder: (context, child, progress) {
          if (progress == null) return child;
          return Stack(
            fit: StackFit.expand,
            children: [
              placeholder,
              Center(
                child: SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    value: progress.expectedTotalBytes == null
                        ? null
                        : (progress.cumulativeBytesLoaded /
                            progress.expectedTotalBytes!),
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}
