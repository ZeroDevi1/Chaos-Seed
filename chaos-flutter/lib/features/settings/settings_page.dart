import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/settings/settings_controller.dart';
import '../../core/settings/settings_model.dart';
import '../music/qq_login_dialog.dart';

class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key, required this.backend});
  final ChaosBackend backend;

  @override
  Widget build(BuildContext context) {
    final s = context.watch<SettingsController>();
    if (!s.loaded) {
      return Platform.isWindows
          ? const fluent.ScaffoldPage(
              content: Center(child: fluent.ProgressRing()))
          : const Scaffold(body: Center(child: CircularProgressIndicator()));
    }

    return Platform.isWindows
        ? _WindowsSettings(backend: backend)
        : _AndroidSettings(backend: backend);
  }
}

class _AndroidSettings extends StatelessWidget {
  const _AndroidSettings({required this.backend});
  final ChaosBackend backend;

  bool _hasQqCookie(AppSettings s) =>
      (s.qqMusicCookieJson ?? '').trim().isNotEmpty;

  @override
  Widget build(BuildContext context) {
    final s = context.watch<SettingsController>();
    final cur = s.settings;

    return Scaffold(
      appBar: AppBar(title: const Text('设置')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          const Text('主题'),
          DropdownButton<AppThemeMode>(
            value: cur.themeMode,
            items: AppThemeMode.values
                .map((m) => DropdownMenuItem(value: m, child: Text(m.name)))
                .toList(),
            onChanged: (v) =>
                s.update(cur.copyWith(themeMode: v ?? cur.themeMode)),
          ),
          const Divider(),
          SwitchListTile(
            title: const Text('自动更新'),
            value: cur.autoUpdateEnabled,
            onChanged: (v) => s.update(cur.copyWith(autoUpdateEnabled: v)),
          ),
          ListTile(
            title: const Text('更新间隔（小时）'),
            subtitle: Text('${cur.autoUpdateIntervalHours}'),
          ),
          const Divider(),
          const Text('QQ 音乐登录'),
          ListTile(
            leading: const Icon(Icons.qr_code),
            title: Text(_hasQqCookie(cur) ? '已登录（Cookie 已缓存）' : '未登录'),
            subtitle: const Text('扫码登录后会缓存 Cookie，用于搜索/下载。'),
            trailing: FilledButton.tonal(
              onPressed: () async {
                final cookie = await showDialog<String?>(
                  context: context,
                  builder: (_) => QqLoginDialog(backend: backend),
                );
                if (cookie == null) return;
                if (!context.mounted) return;
                final next = s.settings.copyWith(qqMusicCookieJson: cookie);
                await s.update(next);
              },
              child: Text(_hasQqCookie(cur) ? '重新登录' : '扫码登录'),
            ),
          ),
          if (_hasQqCookie(cur))
            Align(
              alignment: Alignment.centerRight,
              child: TextButton(
                onPressed: () =>
                    s.update(cur.copyWith(qqMusicCookieJson: null)),
                child: const Text('退出登录（清除 Cookie）'),
              ),
            ),
          const Divider(),
          const Text('音乐下载'),
          ListTile(
            title: const Text('并发'),
            subtitle: Text('${cur.musicDownloadConcurrency}'),
          ),
          ListTile(
            title: const Text('重试'),
            subtitle: Text('${cur.musicDownloadRetries}'),
          ),
          TextFormField(
            initialValue: cur.musicPathTemplate ?? '',
            decoration: const InputDecoration(
              labelText: '下载路径模板（Jinja）',
              helperText: '变量：artist、album、title、ext、track_no（可选）；支持用 / 分隔文件夹。',
            ),
            onChanged: (v) => s.update(
                cur.copyWith(musicPathTemplate: v.trim().isEmpty ? null : v)),
          ),
          const Divider(),
          const SizedBox(height: 12),
        ],
      ),
    );
  }
}

class _WindowsSettings extends StatelessWidget {
  const _WindowsSettings({required this.backend});
  final ChaosBackend backend;

  @override
  Widget build(BuildContext context) {
    final s = context.watch<SettingsController>();
    final cur = s.settings;

    fluent.Widget backendPicker(
        String label, BackendMode curMode, void Function(BackendMode) onSet) {
      return fluent.Row(
        children: [
          fluent.SizedBox(width: 120, child: fluent.Text(label)),
          fluent.ComboBox<BackendMode>(
            value: curMode,
            items: BackendMode.values
                .map((m) =>
                    fluent.ComboBoxItem(value: m, child: fluent.Text(m.name)))
                .toList(),
            onChanged: (v) => onSet(v ?? curMode),
          ),
        ],
      );
    }

    return fluent.ScaffoldPage(
      header: fluent.PageHeader(title: const fluent.Text('设置')),
      content: fluent.Padding(
        padding: const EdgeInsets.all(12),
        child: fluent.Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            fluent.Text('后端：当前=${backend.name}'),
            const fluent.SizedBox(height: 12),
            backendPicker('Live', cur.liveBackendMode,
                (v) => s.update(cur.copyWith(liveBackendMode: v))),
            const fluent.SizedBox(height: 8),
            backendPicker('Music', cur.musicBackendMode,
                (v) => s.update(cur.copyWith(musicBackendMode: v))),
            const fluent.SizedBox(height: 8),
            backendPicker('Lyrics', cur.lyricsBackendMode,
                (v) => s.update(cur.copyWith(lyricsBackendMode: v))),
            const fluent.SizedBox(height: 8),
            backendPicker('Danmaku', cur.danmakuBackendMode,
                (v) => s.update(cur.copyWith(danmakuBackendMode: v))),
            const fluent.SizedBox(height: 16),
            fluent.Text('主题'),
            const fluent.SizedBox(height: 6),
            fluent.ComboBox<AppThemeMode>(
              value: cur.themeMode,
              items: AppThemeMode.values
                  .map((m) =>
                      fluent.ComboBoxItem(value: m, child: fluent.Text(m.name)))
                  .toList(),
              onChanged: (v) =>
                  s.update(cur.copyWith(themeMode: v ?? cur.themeMode)),
            ),
            const fluent.SizedBox(height: 16),
            fluent.Text('自动更新'),
            fluent.ToggleSwitch(
              checked: cur.autoUpdateEnabled,
              onChanged: (v) => s.update(cur.copyWith(autoUpdateEnabled: v)),
            ),
          ],
        ),
      ),
    );
  }
}
