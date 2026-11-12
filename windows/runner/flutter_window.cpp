#include "flutter_window.h"

#include <dwrite.h>
#include <shellapi.h>
#include <shlwapi.h>
#include <wrl/client.h>

#include <cwctype>
#include <filesystem>
#include <map>
#include <optional>
#include <string>
#include <utility>

#include "flutter/generated_plugin_registrant.h"
#include "utils.h"

namespace {

constexpr char kSystemFontsChannelName[] =
    "com.korvect.nauterm/system_fonts";

std::optional<bool> IsDefaultConnectionHandler(const wchar_t* scheme) {
  DWORD length = 0;
  HRESULT status = AssocQueryStringW(ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE,
                                   scheme, nullptr, nullptr, &length);
  if (status == HRESULT_FROM_WIN32(ERROR_NO_ASSOCIATION)) return false;
  if (status != S_FALSE || length == 0) return std::nullopt;
  std::vector<wchar_t> handler(length);
  status = AssocQueryStringW(ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE,
                             scheme, nullptr, handler.data(), &length);
  if (FAILED(status)) return std::nullopt;
  std::vector<wchar_t> current(32768);
  const DWORD current_length = GetModuleFileNameW(
      nullptr, current.data(), static_cast<DWORD>(current.size()));
  if (current_length == 0 || current_length >= current.size()) {
    return std::nullopt;
  }
  std::error_code error;
  const bool matches =
      std::filesystem::equivalent(handler.data(), current.data(), error);
  if (error) return std::nullopt;
  return matches;
}

bool OpenDefaultApplicationSettings() {
  return reinterpret_cast<INT_PTR>(
             ShellExecuteW(nullptr, L"open", L"ms-settings:defaultapps", nullptr,
                           nullptr, SW_SHOWNORMAL)) > 32;
}

std::wstring LowercaseFontFamily(std::wstring family) {
  for (wchar_t& character : family) {
    character = std::towlower(character);
  }
  return family;
}

std::wstring GetFamilyNameAtIndex(IDWriteLocalizedStrings* names,
                                  UINT32 index) {
  UINT32 length = 0;
  if (FAILED(names->GetStringLength(index, &length))) {
    return {};
  }
  std::wstring family(length + 1, L'\0');
  if (FAILED(names->GetString(index, family.data(), length + 1))) {
    return {};
  }
  family.resize(length);
  return family;
}

std::wstring FindFamilyName(IDWriteLocalizedStrings* names,
                            const wchar_t* locale_name) {
  UINT32 index = 0;
  BOOL exists = FALSE;
  names->FindLocaleName(locale_name, &index, &exists);
  return exists ? GetFamilyNameAtIndex(names, index) : std::wstring();
}

std::wstring GetLocalizedFamilyName(IDWriteLocalizedStrings* names) {
  wchar_t locale_name[LOCALE_NAME_MAX_LENGTH] = {};
  if (GetUserDefaultLocaleName(locale_name, LOCALE_NAME_MAX_LENGTH) > 0) {
    std::wstring family = FindFamilyName(names, locale_name);
    if (!family.empty()) {
      return family;
    }
  }
  std::wstring family = FindFamilyName(names, L"en-us");
  if (!family.empty()) {
    return family;
  }
  return names->GetCount() > 0 ? GetFamilyNameAtIndex(names, 0)
                               : std::wstring();
}

bool IsSupportedOutlineFace(IDWriteFont* font) {
  Microsoft::WRL::ComPtr<IDWriteFontFace> face;
  if (FAILED(font->CreateFontFace(&face))) {
    return false;
  }
  switch (face->GetType()) {
    case DWRITE_FONT_FACE_TYPE_CFF:
    case DWRITE_FONT_FACE_TYPE_TRUETYPE:
    case DWRITE_FONT_FACE_TYPE_OPENTYPE_COLLECTION:
    case DWRITE_FONT_FACE_TYPE_RAW_CFF:
      return true;
    default:
      return false;
  }
}

bool HasSupportedOutlineFace(IDWriteFontFamily* family) {
  const UINT32 font_count = family->GetFontCount();
  for (UINT32 index = 0; index < font_count; ++index) {
    Microsoft::WRL::ComPtr<IDWriteFont> font;
    if (FAILED(family->GetFont(index, &font))) {
      continue;
    }
    if (IsSupportedOutlineFace(font.Get())) {
      return true;
    }
  }
  return false;
}

flutter::EncodableList EnumerateSystemFontFamilies(bool monospace_only) {
  Microsoft::WRL::ComPtr<IDWriteFactory> factory;
  if (FAILED(DWriteCreateFactory(
          DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
          reinterpret_cast<IUnknown**>(factory.GetAddressOf())))) {
    return {};
  }

  Microsoft::WRL::ComPtr<IDWriteFontCollection> collection;
  if (FAILED(factory->GetSystemFontCollection(&collection, FALSE))) {
    return {};
  }

  std::map<std::wstring, std::wstring> families;
  const UINT32 family_count = collection->GetFontFamilyCount();
  for (UINT32 index = 0; index < family_count; ++index) {
    Microsoft::WRL::ComPtr<IDWriteFontFamily> family;
    if (FAILED(collection->GetFontFamily(index, &family)) ||
        (monospace_only && !HasSupportedOutlineFace(family.Get()))) {
      continue;
    }

    Microsoft::WRL::ComPtr<IDWriteLocalizedStrings> names;
    if (FAILED(family->GetFamilyNames(&names))) {
      continue;
    }
    std::wstring name = monospace_only
                            ? FindFamilyName(names.Get(), L"en-us")
                            : GetLocalizedFamilyName(names.Get());
    if (name.empty()) {
      name = GetLocalizedFamilyName(names.Get());
    }
    if (!name.empty()) {
      families.emplace(LowercaseFontFamily(name), std::move(name));
    }
  }

  flutter::EncodableList result;
  result.reserve(families.size());
  for (const auto& entry : families) {
    result.emplace_back(Utf8FromUtf16(entry.second.c_str()));
  }
  return result;
}

}  // namespace

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : FlutterWindow(project, true) {}

FlutterWindow::FlutterWindow(const flutter::DartProject& project,
                             bool show_on_first_frame)
    : project_(project), show_on_first_frame_(show_on_first_frame) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  connection_uri_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(),
          "com.korvect.nauterm/connection_uri",
          &flutter::StandardMethodCodec::GetInstance());
  connection_uri_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
        if (call.method_name() == "getDefaultHandlers") {
          flutter::EncodableMap handlers;
          for (const wchar_t* scheme : {L"ssh", L"sftp", L"telnet"}) {
            const auto is_default = IsDefaultConnectionHandler(scheme);
            handlers.emplace(flutter::EncodableValue(Utf8FromUtf16(scheme)),
                             is_default.has_value()
                                 ? flutter::EncodableValue(*is_default)
                                 : flutter::EncodableValue());
          }
          result->Success(flutter::EncodableValue(handlers));
          return;
        }
        if (call.method_name() == "setDefaultHandler") {
          const auto* scheme =
              call.arguments() == nullptr
                  ? nullptr
                  : std::get_if<std::string>(call.arguments());
          if (scheme == nullptr ||
              (*scheme != "ssh" && *scheme != "sftp" && *scheme != "telnet")) {
            result->Error("invalid_scheme", "Unsupported URI scheme.");
            return;
          }
          const std::wstring wide_scheme(scheme->begin(), scheme->end());
          if (IsDefaultConnectionHandler(wide_scheme.c_str()).value_or(false) ||
              OpenDefaultApplicationSettings()) {
            result->Success();
          } else {
            result->Error("set_default_failed",
                          "Unable to open default application settings.");
          }
          return;
        }
        if (call.method_name() != "takePendingUris") {
          result->NotImplemented();
          return;
        }
        flutter::EncodableList uris;
        for (const auto& uri : pending_connection_uris_) {
          uris.emplace_back(uri);
        }
        pending_connection_uris_.clear();
        result->Success(flutter::EncodableValue(uris));
      });
  system_fonts_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(),
          kSystemFontsChannelName,
          &flutter::StandardMethodCodec::GetInstance());
  system_fonts_channel_->SetMethodCallHandler(
      [](const flutter::MethodCall<flutter::EncodableValue>& call,
         std::unique_ptr<
             flutter::MethodResult<flutter::EncodableValue>> result) {
        if (call.method_name() == "listMonospaceFamilies") {
          result->Success(flutter::EncodableValue(
              EnumerateSystemFontFamilies(true)));
        } else if (call.method_name() == "listFontFamilies") {
          result->Success(flutter::EncodableValue(
              EnumerateSystemFontFamilies(false)));
        } else {
          result->NotImplemented();
        }
      });
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  flutter_controller_->engine()->SetNextFrameCallback([this]() {
    if (show_on_first_frame_) {
      this->Show();
    }
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  connection_uri_channel_.reset();
  system_fonts_channel_.reset();
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  if (message == WM_COPYDATA && connection_uri_channel_) {
    const auto* data = reinterpret_cast<const COPYDATASTRUCT*>(lparam);
    if (data == nullptr || data->dwData != kConnectionUriMessage ||
        data->lpData == nullptr || data->cbData < 1 || data->cbData > 65536) {
      return FALSE;
    }
    const char* uri = static_cast<const char*>(data->lpData);
    if (uri[data->cbData - 1] != '\0' ||
        std::char_traits<char>::length(uri) != data->cbData - 1) {
      return FALSE;
    }
    if (data->cbData == 1) {
      connection_uri_channel_->InvokeMethod("activate", nullptr);
    } else {
      pending_connection_uris_.emplace_back(uri, data->cbData - 1);
      connection_uri_channel_->InvokeMethod("urisAvailable", nullptr);
    }
    return TRUE;
  }
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}

void FlutterWindow::QueueConnectionUris(
    const std::vector<std::string>& arguments) {
  for (const auto& argument : arguments) {
    if (argument.find("://") != std::string::npos) {
      pending_connection_uris_.push_back(argument);
    }
  }
}
