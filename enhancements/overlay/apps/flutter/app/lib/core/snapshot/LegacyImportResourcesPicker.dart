// ignore_for_file: file_names

import 'package:flutter/material.dart';

import '../proxy/generated/CoreProxyClients.g.dart';
import 'SnapshotImportUploader.dart';

class LegacyImportResourcesSelection {
  const LegacyImportResourcesSelection({required this.cancelled, this.session});
  final bool cancelled;
  final SnapshotImportSession? session;
}

/// Chooses the optional files omitted by Operit1's own snapshot exporter.
Future<LegacyImportResourcesSelection> pickLegacyImportResources(
  BuildContext context,
  GeneratedCoreProxyClients clients,
) async {
  final addResources = await showDialog<bool>(
    context: context,
    builder: (dialogContext) => AlertDialog(
      title: const Text('补充一代资源'),
      content: const SingleChildScrollView(
        child: Text(
          '一代快照不包含公共 Download/Operit 目录。若需要迁移工作区、工作流或该目录的媒体，请将原目录压缩成 ZIP 并在这里添加。\n\n'
          'ZIP 可以包含 Operit/、Download/Operit/，或直接包含 workspace/、workflow/ 等原目录。工作流 JSON 放在 workflow/ 中，也可以直接选择一个工作流 JSON。\n\n'
          '若你已另行保存一代被排除的私有媒体或模型，可将原 files 目录放入 ZIP 的 files/，缓存放入 cache/；必须保留原相对路径。不要把快照备份 ZIP 再放进资源 ZIP。\n\n'
          '快照与补充资源会一起导入，不覆盖当前已有聊天。未提供的文件无法从快照恢复。',
        ),
      ),
      actions: <Widget>[
        TextButton(onPressed: () => Navigator.pop(dialogContext), child: const Text('取消导入')),
        TextButton(onPressed: () => Navigator.pop(dialogContext, false), child: const Text('仅导入快照')),
        FilledButton(onPressed: () => Navigator.pop(dialogContext, true), child: const Text('添加 ZIP / JSON')),
      ],
    ),
  );
  if (addResources == null || !context.mounted) {
    return const LegacyImportResourcesSelection(cancelled: true);
  }
  if (!addResources) {
    return const LegacyImportResourcesSelection(cancelled: false);
  }
  final file = await SnapshotImportFile.pick(allowJson: true);
  if (file == null) return const LegacyImportResourcesSelection(cancelled: true);
  if (!context.mounted) {
    await file.close();
    return const LegacyImportResourcesSelection(cancelled: true);
  }
  final session = await SnapshotImportUploader(clients).stage(file);
  return LegacyImportResourcesSelection(cancelled: false, session: session);
}
