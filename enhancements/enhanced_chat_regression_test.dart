// Appended to a copy of the upstream transcript regression harness by enhanced_source.py.
void main() {
  upstreamRegressionTests();
  test('enhanced screen stays compatible with the actual generated proxy API', () {
    expect(const AIChatScreen(), isA<Widget>());
  });
  test('first empty selection creates one conversation for concurrent requests', () async {
    final bridge = _BootstrapBridge();
    final vm = _BootstrapViewModel(bridge, null);
    await Future.wait([vm.ensureInitialChat(), vm.ensureInitialChat()]);
    expect(bridge.creations, 1);
  });
  test('initialization does not replace an existing conversation', () async {
    final bridge = _BootstrapBridge();
    await _BootstrapViewModel(bridge, 'existing').ensureInitialChat();
    expect(bridge.creations, 0);
  });
  test('failed first creation can be retried without leaving the shared lock stuck', () async {
    final bridge = _BootstrapBridge()..fail = true;
    final vm = _BootstrapViewModel(bridge, null);
    await expectLater(vm.ensureInitialChat(), throwsStateError);
    bridge.fail = false;
    await vm.ensureInitialChat();
    expect(bridge.creations, 2);
  });
  testWidgets('nested horizontal scroll does not disable transcript follow', (tester) async {
    final controller = ScrollController();
    final horizontal = ScrollController();
    final follow = ValueNotifier<bool>(true);
    await tester.pumpWidget(Stack(textDirection: TextDirection.ltr, children: [
      _enhancedTranscript(controller, follow),
      Positioned(top: 0, left: 0, right: 0, height: 40,
        child: Directionality(textDirection: TextDirection.ltr,
          child: SingleChildScrollView(controller: horizontal,
            scrollDirection: Axis.horizontal,
            child: const SizedBox(width: 2000, height: 40)))),
    ]));
    await tester.pump();
    final context = tester.element(find.byType(ListView).first);
    UserScrollNotification(metrics: horizontal.position,
      context: context, direction: ScrollDirection.forward).dispatch(context);
    expect(follow.value, isTrue);
    await tester.pumpWidget(const SizedBox.shrink());
    controller.dispose(); horizontal.dispose(); follow.dispose();
  });
  testWidgets('programmatic bottom notification cannot resume disabled follow', (tester) async {
    final controller = ScrollController();
    final follow = ValueNotifier<bool>(false);
    await tester.pumpWidget(_enhancedTranscript(controller, follow));
    await tester.pump();
    controller.jumpTo(controller.position.maxScrollExtent);
    await tester.pump();
    expect(follow.value, isFalse);
    await tester.pumpWidget(const SizedBox.shrink());
    controller.dispose(); follow.dispose();
  });
  testWidgets('reading older messages survives newly appended content', (tester) async {
    final controller = ScrollController();
    final follow = ValueNotifier<bool>(true);
    await tester.pumpWidget(_enhancedTranscript(controller, follow));
    for (var n=0; n<5; n++) { await tester.pump(const Duration(milliseconds: 100)); }
    controller.jumpTo(controller.position.maxScrollExtent);
    await tester.pump();
    await tester.drag(find.byType(ListView).first, const Offset(0, 240));
    await tester.pump(const Duration(milliseconds: 100));
    expect(follow.value, isFalse);
    final oldOffset = controller.offset;
    await tester.pumpWidget(_enhancedTranscript(controller, follow, count: 31));
    for (var n=0; n<5; n++) { await tester.pump(const Duration(milliseconds: 100)); }
    expect(follow.value, isFalse);
    expect(controller.offset, lessThan(controller.position.maxScrollExtent-20));
    expect(controller.offset, closeTo(oldOffset, 5));
    await tester.pumpWidget(const SizedBox.shrink());
    controller.dispose(); follow.dispose();
  });
}
Widget _enhancedTranscript(ScrollController controller, ValueNotifier<bool> follow, {int count=30}) {
  return _chatArea(
    messages: List.generate(count, (index) => _aiMessage(timestamp: index+1,
      completedAt: index+2,
      parts: [MessagePart(partId: 'm$index', sequence: 0, kind: 'markdown',
        content: 'Message $index\n\nHistory line\n\nAnother history line',
        toolCallId: null, toolName: null, attributes: const {})])),
    scrollController: controller, autoScrollToBottom: follow,
    isLoading: false, onFollowChanged: (value) => follow.value=value,
  );
}
class _BootstrapViewModel extends ChatViewModel {
  _BootstrapViewModel(_BootstrapBridge b, this.selected) : super(bridge: b);
  final String? selected;
  @override Stream<String?> watchCurrentChatId() => Stream.value(selected);
}
class _BootstrapBridge extends OperitRuntimeBridge {
  int creations=0;
  bool fail=false;
  @override Future<Uint8List> callBytes(CoreCallRequest request) async {
    if (request.methodName != 'createNewChat') {
      throw StateError('Unexpected call: ${request.methodName}');
    }
    creations++;
    await Future<void>.delayed(Duration.zero);
    if (fail) throw StateError('creation failed');
    return encodeCoreLink(<Object?>[0, null]);
  }
  @override Future<CorePushSink> push(CorePushRequest request) => throw UnimplementedError();
  @override Future<CoreEvent> watchSnapshot(CoreWatchRequest request) => throw UnimplementedError();
  @override Stream<CoreEvent> watchStream(CoreWatchRequest request) => throw UnimplementedError();
}
