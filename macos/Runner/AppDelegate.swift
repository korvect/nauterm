import Cocoa
import CoreServices
import Darwin
import FlutterMacOS
import ObjectiveC.runtime
import Sparkle

private typealias NautermRuntimePrepareShutdown = @convention(c) () -> Void

private final class NautermFlutterDartProject: FlutterDartProject {
  // Flutter 3.47 enables Impeller by default on macOS. Its Metal/SDF path is
  // unreliable and significantly slower on some Intel GPUs, while Apple
  // Silicon benefits from keeping the new default.
  @objc var enableImpeller: Bool {
#if arch(x86_64)
    return false
#else
    return true
#endif
  }
}

#if DEBUG
private extension FlutterEngine {
  @objc func nautermEngineCallbackOnPreEngineRestart() {
    // Flutter currently detaches each view controller before closing its window.
    // Resign key while the controller is still attached so the delegate remains valid.
    for window in NSApp.windows where window.isVisible {
      window.orderOut(nil)
    }
    nautermEngineCallbackOnPreEngineRestart()
  }
}

private func installFlutterHotRestartWorkaround() {
  let originalSelector = NSSelectorFromString("engineCallbackOnPreEngineRestart")
  let replacementSelector = #selector(
    FlutterEngine.nautermEngineCallbackOnPreEngineRestart
  )
  guard
    let original = class_getInstanceMethod(FlutterEngine.self, originalSelector),
    let replacement = class_getInstanceMethod(FlutterEngine.self, replacementSelector)
  else {
    return
  }
  method_exchangeImplementations(original, replacement)
}
#endif

@main
class AppDelegate: FlutterAppDelegate {
  private var appMenuChannel: FlutterMethodChannel?
  private var fileDropChannel: FlutterMethodChannel?
  private var systemFontsChannel: FlutterMethodChannel?
  private var externalEditorsChannel: FlutterMethodChannel?
  private var sparkleChannel: FlutterMethodChannel?
  private var titleBarRegionsChannel: FlutterMethodChannel?
  private var sparkleUpdaterController: SPUStandardUpdaterController?
  private var fileDropEnabled = false
  private var titleBarInteractiveRegions: [NSRect] = []
  private var titleBarEventMonitor: Any?

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }

  var engine: FlutterEngine?

  override func applicationDidFinishLaunching(_ notification: Notification) {
#if DEBUG
    installFlutterHotRestartWorkaround()
#endif
    let project = NautermFlutterDartProject()
    engine = FlutterEngine(name: "project", project: project)
    engine?.run(withEntrypoint: nil)
    if let engine {
      RegisterGeneratedPlugins(registry: engine)
      appMenuChannel = FlutterMethodChannel(
        name: "com.korvect.nauterm/app_menu",
        binaryMessenger: engine.binaryMessenger
      )
      fileDropChannel = FlutterMethodChannel(
        name: "com.korvect.nauterm/file_drop",
        binaryMessenger: engine.binaryMessenger
      )
      systemFontsChannel = FlutterMethodChannel(
        name: "com.korvect.nauterm/system_fonts",
        binaryMessenger: engine.binaryMessenger
      )
      externalEditorsChannel = FlutterMethodChannel(
        name: "com.korvect.nauterm/external_editors",
        binaryMessenger: engine.binaryMessenger
      )
      titleBarRegionsChannel = FlutterMethodChannel(
        name: "com.korvect.nauterm/title_bar_regions",
        binaryMessenger: engine.binaryMessenger
      )
      configureSparkle(binaryMessenger: engine.binaryMessenger)
      externalEditorsChannel?.setMethodCallHandler { call, result in
        switch call.method {
        case "listFileApplications":
          let arguments = call.arguments as? [String: Any]
          let fileExtensions = arguments?["extensions"] as? [String] ?? []
          result(Self.fileApplications(fileExtensions: fileExtensions))
        case "fileApplication":
          let arguments = call.arguments as? [String: Any]
          let bundleIdentifier = arguments?["bundleIdentifier"] as? String ?? ""
          result(Self.fileApplication(bundleIdentifier: bundleIdentifier))
        case "chooseFileApplication":
          Self.chooseFileApplication { application in
            result(application)
          }
        case "openFile":
          let arguments = call.arguments as? [String: Any]
          let path = arguments?["path"] as? String ?? ""
          let bundleIdentifier = arguments?["bundleIdentifier"] as? String
          Self.openFile(
            path: path,
            bundleIdentifier: bundleIdentifier,
            result: result
          )
        default:
          result(FlutterMethodNotImplemented)
        }
      }
      systemFontsChannel?.setMethodCallHandler { call, result in
        switch call.method {
        case "listMonospaceFamilies":
          result(Self.monospaceFontFamilies())
        case "listFontFamilies":
          result(Self.fontFamilies())
        default:
          result(FlutterMethodNotImplemented)
        }
      }
      fileDropChannel?.setMethodCallHandler { [weak self] call, result in
        switch call.method {
        case "setEnabled":
          let arguments = call.arguments as? [String: Any]
          let enabled = arguments?["enabled"] as? Bool ?? false
          self?.fileDropEnabled = enabled
          NotificationCenter.default.post(
            name: Notification.Name("com.korvect.nauterm.file_drop_enabled"),
            object: self,
            userInfo: ["enabled": enabled]
          )
          result(nil)
        default:
          result(FlutterMethodNotImplemented)
        }
      }
      titleBarRegionsChannel?.setMethodCallHandler { [weak self] call, result in
        guard call.method == "setInteractiveRegions" else {
          result(FlutterMethodNotImplemented)
          return
        }
        self?.setTitleBarInteractiveRegions(call.arguments)
        result(nil)
      }
      installTitleBarEventMonitor()
      NotificationCenter.default.addObserver(
        self,
        selector: #selector(handleFileDrop(_:)),
        name: Notification.Name("com.korvect.nauterm.file_drop"),
        object: nil
      )
      NotificationCenter.default.addObserver(
        self,
        selector: #selector(handleFullscreenChanged(_:)),
        name: Notification.Name("com.korvect.nauterm.fullscreen_changed"),
        object: nil
      )
    }
  }

  private func configureSparkle(binaryMessenger: FlutterBinaryMessenger) {
    sparkleChannel = FlutterMethodChannel(
      name: "com.korvect.nauterm/sparkle",
      binaryMessenger: binaryMessenger
    )

    let feedURL = Bundle.main.object(forInfoDictionaryKey: "SUFeedURL") as? String
    let publicKey = Bundle.main.object(forInfoDictionaryKey: "SUPublicEDKey") as? String
    if feedURL?.isEmpty == false && publicKey?.isEmpty == false {
      sparkleUpdaterController = SPUStandardUpdaterController(
        startingUpdater: true,
        updaterDelegate: nil,
        userDriverDelegate: nil
      )
    }

    sparkleChannel?.setMethodCallHandler { [weak self] call, result in
      guard call.method == "checkForUpdates" else {
        result(FlutterMethodNotImplemented)
        return
      }
      guard let updaterController = self?.sparkleUpdaterController else {
        result(FlutterError(
          code: "sparkle_not_configured",
          message: "Sparkle requires SUFeedURL and SUPublicEDKey in this build.",
          details: nil
        ))
        return
      }
      updaterController.checkForUpdates(nil)
      result(nil)
    }
  }

  deinit {
    if let titleBarEventMonitor {
      NSEvent.removeMonitor(titleBarEventMonitor)
    }
    NotificationCenter.default.removeObserver(self)
  }

  private func setTitleBarInteractiveRegions(_ arguments: Any?) {
    let values = arguments as? [[String: Any]] ?? []
    titleBarInteractiveRegions = values.compactMap { value in
      guard
        let x = (value["x"] as? NSNumber)?.doubleValue,
        let y = (value["y"] as? NSNumber)?.doubleValue,
        let width = (value["width"] as? NSNumber)?.doubleValue,
        let height = (value["height"] as? NSNumber)?.doubleValue
      else {
        return nil
      }
      return NSRect(
        x: CGFloat(x),
        y: CGFloat(y),
        width: CGFloat(width),
        height: CGFloat(height)
      )
    }
  }

  private func installTitleBarEventMonitor() {
    titleBarEventMonitor = NSEvent.addLocalMonitorForEvents(
      matching: [.mouseMoved, .leftMouseDown, .leftMouseUp]
    ) { [weak self] event in
      self?.updateTitleBarDraggability(for: event)
      return event
    }
  }

  private func updateTitleBarDraggability(for event: NSEvent) {
    guard
      let window = event.window,
      window.titlebarAppearsTransparent,
      window.styleMask.contains(.fullSizeContentView),
      let contentView = window.contentView
    else {
      return
    }

    // The traffic lights remain native, so the green control keeps macOS's
    // Move & Resize / Fill & Arrange menu.
    for buttonType in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
      guard
        let button = window.standardWindowButton(buttonType),
        let buttonSuperview = button.superview
      else {
        continue
      }
      let buttonRect = buttonSuperview.convert(button.frame, to: nil)
      if buttonRect.contains(event.locationInWindow) {
        window.isMovable = true
        return
      }
    }

    let contentPoint = contentView.convert(event.locationInWindow, from: nil)
    let flutterPoint = NSPoint(
      x: contentPoint.x,
      y: contentView.isFlipped ? contentPoint.y : contentView.bounds.height - contentPoint.y
    )
    window.isMovable = !titleBarInteractiveRegions.contains { $0.contains(flutterPoint) }
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return false
  }

  override func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
    appMenuChannel?.invokeMethod("showMainWindow", arguments: nil)
    NSApp.activate(ignoringOtherApps: true)
    return true
  }

  override func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
    guard let appMenuChannel else {
      prepareNativeShutdown()
      return .terminateNow
    }

    appMenuChannel.invokeMethod("requestQuit", arguments: nil) { [weak self] result in
      let shouldQuit = (result as? Bool) ?? true
      if shouldQuit {
        self?.prepareNativeShutdown()
      }
      sender.reply(toApplicationShouldTerminate: shouldQuit)
    }

    return .terminateLater
  }

  override func applicationWillTerminate(_ notification: Notification) {
    prepareNativeShutdown()
  }

  @IBAction func showPreferences(_ sender: Any?) {
    appMenuChannel?.invokeMethod("showSettings", arguments: nil)
  }

  @IBAction func showAbout(_ sender: Any?) {
    appMenuChannel?.invokeMethod("showAbout", arguments: nil)
  }

  @IBAction func checkForUpdates(_ sender: Any?) {
    guard let updaterController = sparkleUpdaterController else {
      let alert = NSAlert()
      alert.alertStyle = .warning
      alert.messageText = "Updates are not configured"
      alert.informativeText = "This build does not include a Sparkle update feed and public key."
      alert.runModal()
      return
    }
    updaterController.checkForUpdates(sender)
  }

  @IBAction func closeSelectedTerminalTab(_ sender: Any?) {
    if NSApp.keyWindow?.title == "Nauterm Settings" {
      NSApp.keyWindow?.performClose(sender)
      return
    }

    appMenuChannel?.invokeMethod("closeSelectedTerminalTab", arguments: nil)
  }

  @objc private func handleFullscreenChanged(_ notification: Notification) {
    let fullscreen = notification.userInfo?["fullscreen"] as? Bool ?? false
    appMenuChannel?.invokeMethod("fullscreenChanged", arguments: fullscreen)
  }

  @objc private func handleFileDrop(_ notification: Notification) {
    guard fileDropEnabled else {
      return
    }
    let method = notification.userInfo?["method"] as? String ?? "filesDropped"
    let paths = notification.userInfo?["paths"] as? [String] ?? []
    if method == "filesDropped" && paths.isEmpty {
      return
    }
    var arguments: [String: Any] = ["paths": paths]
    if let x = notification.userInfo?["x"] as? NSNumber,
       let y = notification.userInfo?["y"] as? NSNumber {
      arguments["x"] = x.doubleValue
      arguments["y"] = y.doubleValue
    }
    fileDropChannel?.invokeMethod(method, arguments: arguments)
  }

  private func prepareNativeShutdown() {
    guard let frameworksPath = Bundle.main.privateFrameworksPath else {
      return
    }
    let libraryPath = "\(frameworksPath)/libnauterm_ffi.dylib"
    guard let handle = dlopen(libraryPath, RTLD_NOW | RTLD_LOCAL) else {
      return
    }
    guard let symbol = dlsym(handle, "nauterm_runtime_prepare_shutdown") else {
      return
    }
    let shutdown = unsafeBitCast(symbol, to: NautermRuntimePrepareShutdown.self)
    shutdown()
  }

  private static func monospaceFontFamilies() -> [String] {
    let manager = NSFontManager.shared
    return manager.availableFontFamilies
      .filter { family in
        guard let font = representativeFont(in: family, manager: manager) else {
          return false
        }
        return font.isFixedPitch ||
          font.fontDescriptor.symbolicTraits.contains(.monoSpace)
      }
      .sorted { left, right in
        left.localizedCaseInsensitiveCompare(right) == .orderedAscending
      }
  }

  private static func fontFamilies() -> [String] {
    return NSFontManager.shared.availableFontFamilies
      .sorted { left, right in
        left.localizedCaseInsensitiveCompare(right) == .orderedAscending
      }
  }

  private static func representativeFont(
    in family: String,
    manager: NSFontManager
  ) -> NSFont? {
    if let font = manager.font(
      withFamily: family,
      traits: [],
      weight: 5,
      size: 12
    ) {
      return font
    }
    guard let members = manager.availableMembers(ofFontFamily: family) else {
      return nil
    }
    for member in members {
      guard let postScriptName = member.first as? String else {
        continue
      }
      if let font = NSFont(name: postScriptName, size: 12) {
        return font
      }
    }
    return nil
  }

  private static func fileApplications(
    fileExtensions: [String]
  ) -> [[String: Any]] {
    var bundleIdentifiers = Set<String>()
    var defaultBundleIdentifiers = Set<String>()

    for pathExtension in fileExtensions {
      guard let unmanagedType = UTTypeCreatePreferredIdentifierForTag(
        kUTTagClassFilenameExtension,
        pathExtension as CFString,
        nil
      ) else {
        continue
      }
      let contentType = unmanagedType.takeRetainedValue()
      appendHandlers(for: contentType, to: &bundleIdentifiers)
      if let defaultHandler = LSCopyDefaultRoleHandlerForContentType(
        contentType,
        .all
      ) {
        defaultBundleIdentifiers.insert(
          defaultHandler.takeRetainedValue() as String
        )
      }
    }

    let ownBundleIdentifier = Bundle.main.bundleIdentifier
    return bundleIdentifiers.compactMap { bundleIdentifier in
      guard bundleIdentifier != ownBundleIdentifier,
            let applicationURL = NSWorkspace.shared.urlForApplication(
              withBundleIdentifier: bundleIdentifier
            )
      else {
        return nil
      }
      return fileApplication(
        at: applicationURL,
        bundleIdentifier: bundleIdentifier,
        isDefault: defaultBundleIdentifiers.contains(bundleIdentifier)
      )
    }.sorted { left, right in
      let leftDefault = left["isDefault"] as? Bool ?? false
      let rightDefault = right["isDefault"] as? Bool ?? false
      if leftDefault != rightDefault {
        return leftDefault
      }
      let leftName = left["name"] as? String ?? ""
      let rightName = right["name"] as? String ?? ""
      return leftName.localizedCaseInsensitiveCompare(rightName) == .orderedAscending
    }
  }

  private static func fileApplication(
    bundleIdentifier: String
  ) -> [String: Any]? {
    guard !bundleIdentifier.isEmpty,
          bundleIdentifier != Bundle.main.bundleIdentifier,
          let applicationURL = NSWorkspace.shared.urlForApplication(
            withBundleIdentifier: bundleIdentifier
          )
    else {
      return nil
    }
    return fileApplication(
      at: applicationURL,
      bundleIdentifier: bundleIdentifier,
      isDefault: false
    )
  }

  private static func chooseFileApplication(
    completion: @escaping ([String: Any]?) -> Void
  ) {
    let panel = NSOpenPanel()
    panel.directoryURL = URL(fileURLWithPath: "/Applications")
    panel.canChooseFiles = true
    panel.canChooseDirectories = false
    panel.allowsMultipleSelection = false
    panel.allowedFileTypes = ["app"]

    let handleResponse: (NSApplication.ModalResponse) -> Void = { response in
      guard response == .OK,
            let applicationURL = panel.url,
            let bundleIdentifier = Bundle(url: applicationURL)?.bundleIdentifier
      else {
        completion(nil)
        return
      }
      completion(
        fileApplication(
          at: applicationURL,
          bundleIdentifier: bundleIdentifier,
          isDefault: false
        )
      )
    }
    if let window = NSApp.keyWindow {
      panel.beginSheetModal(for: window, completionHandler: handleResponse)
    } else {
      panel.begin(completionHandler: handleResponse)
    }
  }

  private static func openFile(
    path: String,
    bundleIdentifier: String?,
    result: @escaping FlutterResult
  ) {
    guard !path.isEmpty else {
      result(
        FlutterError(
          code: "invalid_path",
          message: "The file path is empty.",
          details: nil
        )
      )
      return
    }

    let fileURL = URL(fileURLWithPath: path)
    guard let bundleIdentifier else {
      if NSWorkspace.shared.open(fileURL) {
        result(nil)
      } else {
        result(
          FlutterError(
            code: "open_failed",
            message: "The system default application could not open the file.",
            details: path
          )
        )
      }
      return
    }

    guard let applicationURL = NSWorkspace.shared.urlForApplication(
      withBundleIdentifier: bundleIdentifier
    ) else {
      result(
        FlutterError(
          code: "application_not_found",
          message: "The selected application is no longer installed.",
          details: bundleIdentifier
        )
      )
      return
    }

    NSWorkspace.shared.open(
      [fileURL],
      withApplicationAt: applicationURL,
      configuration: NSWorkspace.OpenConfiguration()
    ) { _, error in
      if let error {
        result(
          FlutterError(
            code: "open_failed",
            message: error.localizedDescription,
            details: path
          )
        )
      } else {
        result(nil)
      }
    }
  }

  private static func fileApplication(
    at applicationURL: URL,
    bundleIdentifier: String,
    isDefault: Bool
  ) -> [String: Any] {
    let bundle = Bundle(url: applicationURL)
    let localizedFileName = FileManager.default.displayName(
      atPath: applicationURL.path
    )
    let localizedApplicationName = (localizedFileName as NSString)
      .deletingPathExtension
    let preferredDisplayName = localizedApplicationName.isEmpty
      ? nil
      : localizedApplicationName
    let displayName =
      preferredDisplayName
      ?? bundle?.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String
      ?? bundle?.object(forInfoDictionaryKey: "CFBundleName") as? String
      ?? applicationURL.deletingPathExtension().lastPathComponent
    var application: [String: Any] = [
      "name": displayName,
      "bundleIdentifier": bundleIdentifier,
      "path": applicationURL.path,
      "isDefault": isDefault,
    ]
    if let iconData = pngData(
      for: NSWorkspace.shared.icon(forFile: applicationURL.path),
      size: NSSize(width: 32, height: 32)
    ) {
      application["icon"] = FlutterStandardTypedData(bytes: iconData)
    }
    return application
  }

  private static func appendHandlers(
    for contentType: CFString,
    to bundleIdentifiers: inout Set<String>
  ) {
    guard let handlers = LSCopyAllRoleHandlersForContentType(
      contentType,
      .all
    )?.takeRetainedValue() as? [String] else {
      return
    }
    bundleIdentifiers.formUnion(handlers)
  }

  private static func pngData(for image: NSImage, size: NSSize) -> Data? {
    let rendered = NSImage(size: size)
    rendered.lockFocus()
    NSGraphicsContext.current?.imageInterpolation = .high
    image.draw(
      in: NSRect(origin: .zero, size: size),
      from: NSRect(origin: .zero, size: image.size),
      operation: .copy,
      fraction: 1
    )
    rendered.unlockFocus()
    guard let tiffData = rendered.tiffRepresentation,
          let bitmap = NSBitmapImageRep(data: tiffData)
    else {
      return nil
    }
    return bitmap.representation(using: .png, properties: [:])
  }
}
