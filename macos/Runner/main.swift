import AppKit

// Hand slash-connect args to the primary instance before NSApplication /
// MainMenu nib load. Any earlier AppKit touch shows a second Dock flash.
_ = EphemeralConnectLauncher.tryForwardAndExitIfNeeded()
_ = NSApplicationMain(CommandLine.argc, CommandLine.unsafeArgv)
