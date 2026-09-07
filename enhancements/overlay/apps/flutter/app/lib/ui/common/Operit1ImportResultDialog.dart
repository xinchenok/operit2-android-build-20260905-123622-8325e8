// ignore_for_file: file_names

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../core/logging/ClientLogger.dart';
import '../../core/proxy/generated/CoreProxyModels.g.dart';
import '../../core/snapshot/SnapshotImportUploader.dart';

/// Temporary upload cleanup must not turn an accepted import into a retry.
Future<void> discardOperit1ImportSession(SnapshotImportSession? session) async {
  if (session == null) return;
  try {
    await session.discard();
  } catch (error, stackTrace) {
    ClientLogger.e(
      'snapshot temporary upload cleanup failed',
      tag: 'Operit1SnapshotImport',
      error: error,
      stackTrace: stackTrace,
    );
  }
}

/// Shows the actual migration result, including every nonempty migration note.
Future<void> showOperit1ImportResultDialog(
  BuildContext context,
  Operit1SnapshotImportResult result,
) async {
  if (!context.mounted) return;
  final notes = result.modelConfig.skippedFields
      .map((note) => note.trim())
      .where((note) => note.isNotEmpty)
      .toSet()
      .toList();
  final summary = <String>[
    '聊天：${result.importedChats} 个，消息：${result.importedMessages} 条',
    '模型：${result.modelConfig.importedModelCount} 个',
    '记忆：${result.importedMemories} 条，关联：${result.importedMemoryLinks} 条',
    '工作区：${result.importedWorkspaces} 个',
    '资源文件：${result.importedFiles + result.importedExternalFiles + result.importedWorkspaceFiles} 个',
    if (notes.isNotEmpty) '\n迁移说明：\n${notes.join('\n\n')}',
  ].join('\n');
  await showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (dialogContext) => AlertDialog(
      title: const Text('导入结果'),
      content: SingleChildScrollView(child: SelectableText(summary)),
      actions: <Widget>[
        TextButton.icon(
          onPressed: () async {
            try {
              await Clipboard.setData(ClipboardData(text: summary));
              if (dialogContext.mounted) {
                ScaffoldMessenger.of(dialogContext).showSnackBar(
                  const SnackBar(content: Text('已复制导入结果')),
                );
              }
            } catch (_) {
              if (dialogContext.mounted) {
                ScaffoldMessenger.of(dialogContext).showSnackBar(
                  const SnackBar(content: Text('复制失败，可长按结果文字复制')),
                );
              }
            }
          },
          icon: const Icon(Icons.copy_rounded),
          label: const Text('复制结果'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(dialogContext).pop(),
          child: const Text('完成'),
        ),
      ],
    ),
  );
}
