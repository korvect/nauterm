#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include "flutter_window.h"
#include "utils.h"

namespace {

bool ForwardConnectionUris(const std::vector<std::string>& arguments) {
  HWND existing = nullptr;
  // Another process can still be creating its hidden runner window.
  for (int attempt = 0; attempt < 50 && existing == nullptr; ++attempt) {
    existing = FindWindowW(L"FLUTTER_RUNNER_WIN32_WINDOW", L"nauterm");
    if (existing == nullptr) Sleep(100);
  }
  if (existing == nullptr) return false;
  DWORD process_id = 0;
  GetWindowThreadProcessId(existing, &process_id);
  AllowSetForegroundWindow(process_id);
  bool sent_uri = false;
  for (const auto& argument : arguments) {
    if (argument.find("://") == std::string::npos) continue;
    if (argument.size() >= 65536) return false;
    COPYDATASTRUCT data{};
    data.dwData = FlutterWindow::kConnectionUriMessage;
    data.cbData = static_cast<DWORD>(argument.size() + 1);
    data.lpData = const_cast<char*>(argument.c_str());
    DWORD_PTR result = 0;
    if (!SendMessageTimeoutW(existing, WM_COPYDATA, 0,
                             reinterpret_cast<LPARAM>(&data),
                             SMTO_ABORTIFHUNG | SMTO_BLOCK, 5000, &result) ||
        result == FALSE) return false;
    sent_uri = true;
  }
  if (sent_uri) return true;
  char empty[] = "";
  COPYDATASTRUCT data{FlutterWindow::kConnectionUriMessage, 1, empty};
  DWORD_PTR result = 0;
  return SendMessageTimeoutW(existing, WM_COPYDATA, 0,
                            reinterpret_cast<LPARAM>(&data),
                            SMTO_ABORTIFHUNG | SMTO_BLOCK, 5000, &result) &&
         result != FALSE;
}

}  // namespace

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  const auto arguments = GetCommandLineArguments();
  HANDLE instance_mutex = CreateMutexW(
      nullptr, FALSE, L"Local\\com.korvect.nauterm.instance");
  if (instance_mutex == nullptr) return EXIT_FAILURE;
  if (GetLastError() == ERROR_ALREADY_EXISTS) {
    const bool forwarded = ForwardConnectionUris(arguments);
    CloseHandle(instance_mutex);
    return forwarded ? EXIT_SUCCESS : EXIT_FAILURE;
  }
  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  flutter::DartProject project(L"data");

  project.set_dart_entrypoint_arguments(arguments);

  FlutterWindow window(project, false);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size(1280, 720);
  if (!window.Create(L"nauterm", origin, size)) {
    CloseHandle(instance_mutex);
    return EXIT_FAILURE;
  }
  window.QueueConnectionUris(arguments);
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  CloseHandle(instance_mutex);
  return EXIT_SUCCESS;
}
