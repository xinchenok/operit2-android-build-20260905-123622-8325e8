// ignore_for_file: file_names

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../core/logging/ClientLogger.dart';
import '../../core/runtime/RuntimeBootstrapManager.dart';

/// Shows the enhanced distribution notice once per application data lifetime.
class EnhancedDisclaimerGate extends StatefulWidget {
  const EnhancedDisclaimerGate({super.key, required this.child});

  final Widget child;

  @override
  State<EnhancedDisclaimerGate> createState() => _EnhancedDisclaimerGateState();
}

class _EnhancedDisclaimerGateState extends State<EnhancedDisclaimerGate>
    with WidgetsBindingObserver {
  static const String _confirmation =
      '我同意这是第三方版本，与官方无关，我不会向官方反馈此版本的问题';
  static const Duration _requiredReadingTime = Duration(seconds: 5);

  final TextEditingController _controller = TextEditingController();
  final Stopwatch _visibleReadingTime = Stopwatch();
  Timer? _timer;
  late bool _accepted;
  bool _saving = false;
  int _remainingSeconds = 5;
  String? _error;

  bool get _canAccept =>
      !_saving &&
      _visibleReadingTime.elapsed >= _requiredReadingTime &&
      WidgetsBinding.instance.lifecycleState == AppLifecycleState.resumed &&
      _controller.text.trim() == _confirmation;

  @override
  void initState() {
    super.initState();
    _accepted = RuntimeBootstrapManager.instance.config.enhancedDisclaimerAccepted;
    WidgetsBinding.instance.addObserver(this);
    _controller.addListener(_refreshInput);
    if (!_accepted) {
      _resumeReadingAfterFrame();
    }
  }

  void _refreshInput() {
    if (mounted) {
      setState(() {});
    }
  }

  /// Only time spent with this page rendered in the foreground counts.
  void _resumeReadingAfterFrame() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted ||
          _accepted ||
          WidgetsBinding.instance.lifecycleState != AppLifecycleState.resumed) {
        return;
      }
      if (_visibleReadingTime.elapsed < _requiredReadingTime) {
        _visibleReadingTime.start();
        _timer?.cancel();
        _timer = Timer.periodic(const Duration(milliseconds: 100), (_) {
          if (!mounted) {
            return;
          }
          final remainingMilliseconds =
              _requiredReadingTime.inMilliseconds -
              _visibleReadingTime.elapsedMilliseconds;
          final remaining = remainingMilliseconds <= 0
              ? 0
              : (remainingMilliseconds / 1000).ceil();
          if (remaining != _remainingSeconds) {
            setState(() => _remainingSeconds = remaining);
          }
          if (remaining == 0) {
            _visibleReadingTime.stop();
            _timer?.cancel();
            _timer = null;
          }
        });
      }
      setState(() {
        if (_visibleReadingTime.elapsed >= _requiredReadingTime) {
          _remainingSeconds = 0;
        }
      });
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    _visibleReadingTime.stop();
    _timer?.cancel();
    _timer = null;
    if (state == AppLifecycleState.resumed && !_accepted) {
      _resumeReadingAfterFrame();
    } else if (mounted && !_accepted) {
      setState(() {});
    }
  }

  Future<void> _copyConfirmation() async {
    try {
      await Clipboard.setData(const ClipboardData(text: _confirmation));
    } on PlatformException catch (error) {
      if (mounted) {
        setState(() => _error = '复制失败，请手动输入确认文字：${error.message}');
      }
    }
  }

  Future<void> _pasteConfirmation() async {
    try {
      final data = await Clipboard.getData(Clipboard.kTextPlain);
      if (!mounted || _accepted || _saving) {
        return;
      }
      if (data?.text case final String text) {
        _controller.value = TextEditingValue(
          text: text,
          selection: TextSelection.collapsed(offset: text.length),
        );
      }
    } on PlatformException catch (error) {
      if (mounted) {
        setState(() => _error = '粘贴失败，请手动输入确认文字：${error.message}');
      }
    }
  }

  Future<void> _accept() async {
    if (!_canAccept) {
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await RuntimeBootstrapManager.instance.acceptEnhancedDisclaimer();
      if (mounted) {
        setState(() => _accepted = true);
      }
    } catch (error, stackTrace) {
      ClientLogger.e(
        'Failed to persist enhanced distribution notice acknowledgement',
        tag: 'EnhancedDisclaimer',
        error: error,
        stackTrace: stackTrace,
      );
      if (mounted) {
        setState(() => _error = '确认记录保存失败，请重试。原有数据不会被清除。');
      }
    } finally {
      if (mounted) {
        setState(() => _saving = false);
      }
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _timer?.cancel();
    _visibleReadingTime.stop();
    _controller.removeListener(_refreshInput);
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_accepted) {
      return widget.child;
    }
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: ThemeData(colorSchemeSeed: const Color(0xff51657a)),
      darkTheme: ThemeData(
        brightness: Brightness.dark,
        colorSchemeSeed: const Color(0xff51657a),
      ),
      home: PopScope(
        canPop: false,
        child: Scaffold(
          body: SafeArea(
            child: Center(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(24),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 560),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      const Icon(Icons.info_outline_rounded, size: 44),
                      const SizedBox(height: 20),
                      const Text(
                        '第三方增强版使用声明',
                        textAlign: TextAlign.center,
                        style: TextStyle(fontSize: 26, fontWeight: FontWeight.w700),
                      ),
                      const SizedBox(height: 24),
                      const Text(
                        '你正在使用由第三方修改、构建和签名的 Operit2 增强版，'
                        '其中包含体验修复及旧版工具包适配。\n\n'
                        '本版本不是 Operit 官方发布的版本，与官方团队无关，'
                        '也不代表官方的支持或认可。\n\n'
                        '遇到本版本的问题，请反馈到第三方构建仓库 '
                        'xinchenok/operit2-android-cx，'
                        '不要向 Operit 官方反馈此版本的问题。\n\n'
                        '请阅读满 5 秒，并在下方输入完整确认文字。'
                        '可以复制、粘贴；切到后台的时间不计入阅读时间。',
                        style: TextStyle(fontSize: 16, height: 1.6),
                      ),
                      const SizedBox(height: 24),
                      const SelectableText(_confirmation),
                      const SizedBox(height: 8),
                      Wrap(
                        spacing: 8,
                        children: <Widget>[
                          TextButton.icon(
                            onPressed: _saving ? null : _copyConfirmation,
                            icon: const Icon(Icons.copy_rounded),
                            label: const Text('复制确认文字'),
                          ),
                          TextButton.icon(
                            onPressed: _saving ? null : _pasteConfirmation,
                            icon: const Icon(Icons.content_paste_rounded),
                            label: const Text('粘贴到输入框'),
                          ),
                        ],
                      ),
                      const SizedBox(height: 8),
                      TextField(
                        controller: _controller,
                        enabled: !_saving,
                        minLines: 2,
                        maxLines: 4,
                        decoration: const InputDecoration(
                          border: OutlineInputBorder(),
                          labelText: '输入完整确认文字',
                          alignLabelWithHint: true,
                        ),
                      ),
                      if (_error != null) ...<Widget>[
                        const SizedBox(height: 12),
                        Text(_error!),
                      ],
                      const SizedBox(height: 20),
                      FilledButton(
                        onPressed: _canAccept ? _accept : null,
                        child: Text(
                          _saving
                              ? '正在保存'
                              : _remainingSeconds > 0
                              ? '请先阅读，还需 $_remainingSeconds 秒'
                              : '我已理解并同意，进入应用',
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
