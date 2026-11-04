/* Nauterm's CMD integration, limited to the CMD child it launches.
 *
 * CMD has no preexec hook. Intercept its own cooked console reads and writes,
 * retain the exact marked prompt and request errorlevel as a separate input
 * line. User input is never wrapped, re-expanded, or appended to a probe.
 * Child applications keep their original imports and console semantics.
 */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#ifdef NAUTERM_LAUNCHER

static BOOL WINAPI ignore_control(DWORD event) {
    return event == CTRL_C_EVENT || event == CTRL_BREAK_EVENT;
}

int wmain(int argc, wchar_t **argv) {
    wchar_t dll[MAX_PATH * 4], command[MAX_PATH * 4];
    STARTUPINFOW startup;
    PROCESS_INFORMATION process;
    DWORD result = 1;
    HANDLE loader = NULL;
    void *remote = NULL;
    if (argc != 2 || !GetModuleFileNameW(NULL, dll, ARRAYSIZE(dll))) return 1;
    wchar_t *slash = wcsrchr(dll, L'\\');
    if (!slash) return 1;
    wcscpy_s(slash + 1, ARRAYSIZE(dll) - (slash + 1 - dll), L"nauterm-cmd-hook.dll");
    if (swprintf_s(command, ARRAYSIZE(command), L"\"%s\"", argv[1]) < 0) return 1;
    ZeroMemory(&startup, sizeof(startup));
    ZeroMemory(&process, sizeof(process));
    startup.cb = sizeof(startup);
    SECURITY_ATTRIBUTES security = { sizeof(security), NULL, TRUE };
    startup.dwFlags = STARTF_USESTDHANDLES;
    startup.hStdInput = CreateFileW(L"CONIN$", GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE, &security, OPEN_EXISTING, 0, NULL);
    startup.hStdOutput = CreateFileW(L"CONOUT$", GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE, &security, OPEN_EXISTING, 0, NULL);
    startup.hStdError = startup.hStdOutput;
    SetConsoleCtrlHandler(ignore_control, TRUE);
    if (!CreateProcessW(argv[1], command, NULL, NULL, TRUE, CREATE_SUSPENDED,
                        NULL, NULL, &startup, &process)) goto failed;
    CloseHandle(startup.hStdInput);
    CloseHandle(startup.hStdOutput);
    SIZE_T bytes = (wcslen(dll) + 1) * sizeof(wchar_t);
    remote = VirtualAllocEx(process.hProcess, NULL, bytes, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if (!remote || !WriteProcessMemory(process.hProcess, remote, dll, bytes, NULL)) goto failed;
    loader = CreateRemoteThread(process.hProcess, NULL, 0,
        (LPTHREAD_START_ROUTINE)(void *)GetProcAddress(GetModuleHandleW(L"kernel32.dll"), "LoadLibraryW"),
        remote, 0, NULL);
    if (!loader || WaitForSingleObject(loader, 10000) != WAIT_OBJECT_0) goto failed;
    DWORD loaded = 0;
    if (!GetExitCodeThread(loader, &loaded) || !loaded) goto failed;
    CloseHandle(loader);
    loader = NULL;
    VirtualFreeEx(process.hProcess, remote, 0, MEM_RELEASE);
    remote = NULL;
    if (ResumeThread(process.hThread) == (DWORD)-1) goto failed;
    WaitForSingleObject(process.hProcess, INFINITE);
    GetExitCodeProcess(process.hProcess, &result);
    CloseHandle(process.hThread);
    CloseHandle(process.hProcess);
    return (int)result;
failed:
    fprintf(stderr, "Nauterm could not initialize CMD console integration (Windows error %lu).\n", GetLastError());
    if (loader) CloseHandle(loader);
    if (process.hProcess) {
        TerminateProcess(process.hProcess, 1);
        if (remote) VirtualFreeEx(process.hProcess, remote, 0, MEM_RELEASE);
        CloseHandle(process.hThread);
        CloseHandle(process.hProcess);
    }
    return 1;
}

#else

typedef BOOL (WINAPI *ReadConsoleFn)(HANDLE, LPVOID, DWORD, LPDWORD, PCONSOLE_READCONSOLE_CONTROL);
typedef BOOL (WINAPI *WriteConsoleFn)(HANDLE, const void *, DWORD, LPDWORD, LPVOID);
typedef BOOL (WINAPI *WriteFileFn)(HANDLE, LPCVOID, DWORD, LPDWORD, LPOVERLAPPED);
typedef BOOL (WINAPI *SetEnvFn)(LPCWSTR, LPCWSTR);
static ReadConsoleFn real_read;
static WriteConsoleFn real_write;
static WriteFileFn real_file;
static SetEnvFn real_setenv;

static wchar_t token[129], marker[256], prefix[256];
static size_t marker_length, pending_length, prompt_length, probe_length;
static wchar_t pending[256], prompt[65536], probe[512];
static BOOL capturing, active, probing, ai_armed, ready;
static DWORD ai_remaining;
static int ai_status;
static HANDLE output;
static DWORD shell_pid;

static void write_wide(const wchar_t *text, size_t length) {
    DWORD written;
    while (length) {
        DWORD count = (DWORD)(length > 16384 ? 16384 : length);
        if (!real_write(output, text, count, &written, NULL) || !written) break;
        length -= written;
        text += written;
    }
}

static void emit(const wchar_t *text) { write_wide(text, wcslen(text)); }

static void osc(const wchar_t *payload) {
    emit(L"\x1b]"); emit(payload); emit(L"\a");
}

static void event_error(void) {
    wchar_t message[256];
    swprintf_s(message, ARRAYSIZE(message), L"777;nauterm-integration-error=%s", token);
    osc(message);
    active = FALSE;
    ai_armed = FALSE;
}

static void cwd_event(void) {
    wchar_t directory[32768];
    DWORD length = GetCurrentDirectoryW(ARRAYSIZE(directory), directory);
    if (!length || length >= ARRAYSIZE(directory)) return;
    int bytes = WideCharToMultiByte(CP_UTF8, 0, directory, (int)length, NULL, 0, NULL, NULL);
    if (!bytes) return;
    char *utf8 = (char *)malloc((size_t)bytes);
    if (!utf8) return;
    WideCharToMultiByte(CP_UTF8, 0, directory, (int)length, utf8, bytes, NULL, NULL);
    emit(L"\x1b]7;file://");
    if (!(length > 1 && directory[0] == '\\' && directory[1] == '\\')) emit(L"/");
    for (int i = directory[0] == '\\' && directory[1] == '\\' ? 2 : 0; i < bytes; ++i) {
        unsigned char ch = (unsigned char)utf8[i];
        wchar_t unit[4];
        if (ch == '\\') emit(L"/");
        else if ((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') ||
                 (ch >= '0' && ch <= '9') || ch == ':' || ch == '/' || ch == '-' || ch == '_' || ch == '.' || ch == '~') {
            unit[0] = ch; unit[1] = 0; emit(unit);
        } else {
            swprintf_s(unit, ARRAYSIZE(unit), L"%%%02X", ch); emit(unit);
        }
    }
    emit(L"\a");
    free(utf8);
}

static void prompt_start(void) {
    wchar_t payload[256];
    if (!ready) {
        swprintf_s(payload, ARRAYSIZE(payload), L"777;nauterm-integration-ready=%s", token);
        osc(payload);
        ready = TRUE;
    }
    cwd_event();
    swprintf_s(payload, ARRAYSIZE(payload), L"133;A;cl=line;aid=%lu", shell_pid);
    osc(payload);
}

static void prompt_end(void) {
    wchar_t payload[128];
    swprintf_s(payload, ARRAYSIZE(payload), L"133;B;aid=%lu", shell_pid);
    osc(payload);
}

static BOOL parse_status(int *status) {
    wchar_t expected[256];
    swprintf_s(expected, ARRAYSIZE(expected), L"__nauterm_exit_%s;", token);
    probe[probe_length] = 0;
    wchar_t *value = probe;
    while (*value == '\r' || *value == '\n') ++value;
    size_t length = wcslen(expected);
    if (wcsncmp(value, expected, length)) return FALSE;
    value += length;
    wchar_t *end;
    __int64 code = _wcstoi64(value, &end, 10);
    if (end == value || code < INT_MIN || code > INT_MAX) return FALSE;
    while (*end == '\r' || *end == '\n') ++end;
    if (*end) return FALSE;
    *status = (int)code;
    return TRUE;
}

static void complete(void) {
    int status;
    wchar_t payload[256];
    if (!parse_status(&status)) { event_error(); return; }
    swprintf_s(payload, ARRAYSIZE(payload), L"4545;CommandExited;%lu;%d", shell_pid, status);
    osc(payload);
    swprintf_s(payload, ARRAYSIZE(payload), L"133;D;%d;aid=%lu", status, shell_pid);
    osc(payload);
    ai_status = status;
    if (ai_armed && !ai_remaining) {
        swprintf_s(payload, ARRAYSIZE(payload), L"777;nauterm-command-end=%s;%d", token, status);
        osc(payload);
        ai_armed = FALSE;
    }
    active = FALSE;
}

static void visible(wchar_t ch) {
    if (capturing) {
        if (prompt_length < ARRAYSIZE(prompt)) { prompt[prompt_length++] = ch; return; }
        // Bounded capture: pass unusually large prompts through without
        // treating the following read as command input.
        write_wide(prompt, prompt_length);
        prompt_length = 0;
        capturing = FALSE;
    }
    if (probing) {
        if (probe_length < ARRAYSIZE(probe) - 1) probe[probe_length++] = ch;
        return;
    }
    write_wide(&ch, 1);
}

static void gate(const wchar_t *text, size_t length) {
    for (size_t i = 0; i < length; ++i) {
        if (!pending_length && !capturing && !probing && text[i] != marker[0]) {
            size_t start = i;
            while (i < length && text[i] != marker[0]) ++i;
            write_wide(text + start, i - start);
            if (i == length) break;
        }
        pending[pending_length++] = text[i];
        while (pending_length && wmemcmp(pending, marker, pending_length)) {
            visible(pending[0]);
            --pending_length;
            memmove(pending, pending + 1, pending_length * sizeof(wchar_t));
        }
        if (pending_length == marker_length) {
            if (capturing) write_wide(prompt, prompt_length);
            capturing = TRUE;
            prompt_length = 0;
            pending_length = 0;
        }
    }
}

static BOOL WINAPI hooked_write(HANDLE handle, const void *buffer, DWORD count, LPDWORD written, LPVOID reserved) {
    DWORD mode;
    if (!GetConsoleMode(handle, &mode)) return real_write(handle, buffer, count, written, reserved);
    output = handle;
    gate((const wchar_t *)buffer, count);
    if (written) *written = count;
    return TRUE;
}

static BOOL WINAPI hooked_file(HANDLE handle, LPCVOID buffer, DWORD count, LPDWORD written, LPOVERLAPPED overlapped) {
    DWORD mode;
    if (overlapped || !GetConsoleMode(handle, &mode)) return real_file(handle, buffer, count, written, overlapped);
    UINT cp = GetConsoleOutputCP();
    int length = MultiByteToWideChar(cp, 0, (const char *)buffer, count, NULL, 0);
    if (!length) return real_file(handle, buffer, count, written, overlapped);
    wchar_t *wide = (wchar_t *)malloc((size_t)length * sizeof(wchar_t));
    if (!wide) return real_file(handle, buffer, count, written, overlapped);
    MultiByteToWideChar(cp, 0, (const char *)buffer, count, wide, length);
    output = handle;
    gate(wide, (size_t)length);
    free(wide);
    if (written) *written = count;
    return TRUE;
}

static void begin(const wchar_t *line, DWORD count) {
    while (count && (line[count - 1] == '\r' || line[count - 1] == '\n')) --count;
    DWORD nonspace = 0;
    while (nonspace < count && (line[nonspace] == ' ' || line[nonspace] == '\t')) ++nonspace;
    if (nonspace == count) return;
    int bytes = WideCharToMultiByte(CP_UTF8, 0, line, count, NULL, 0, NULL, NULL);
    char *utf8 = (char *)malloc(bytes);
    if (!utf8) { event_error(); return; }
    WideCharToMultiByte(CP_UTF8, 0, line, count, utf8, bytes, NULL, NULL);
    wchar_t payload[256];
    swprintf_s(payload, ARRAYSIZE(payload), L"\x1b]4545;CommandStarted;%lu;", shell_pid);
    emit(payload);
    const wchar_t *alphabet = L"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (int i = 0; i < bytes; i += 3) {
        unsigned int n = (unsigned char)utf8[i] << 16;
        if (i + 1 < bytes) n |= (unsigned char)utf8[i + 1] << 8;
        if (i + 2 < bytes) n |= (unsigned char)utf8[i + 2];
        wchar_t encoded[4] = { alphabet[(n >> 18) & 63], alphabet[(n >> 12) & 63],
            i + 1 < bytes ? alphabet[(n >> 6) & 63] : '=', i + 2 < bytes ? alphabet[n & 63] : '=' };
        write_wide(encoded, 4);
    }
    free(utf8);
    emit(L"\a");
    swprintf_s(payload, ARRAYSIZE(payload), L"133;C;aid=%lu", shell_pid);
    osc(payload);
    active = TRUE;
}

static void consumed_ai_lines(const wchar_t *line, DWORD count) {
    if (!ai_armed) return;
    for (DWORD i = 0; i < count; ++i) {
        if ((line[i] == '\n' || (line[i] == '\r' && (i + 1 == count || line[i + 1] != '\n'))) && ai_remaining)
            --ai_remaining;
    }
}

static BOOL read_ai_header(HANDLE handle) {
    wchar_t header[256], expected[256];
    DWORD mode, read;
    GetConsoleMode(handle, &mode);
    SetConsoleMode(handle, mode & ~ENABLE_ECHO_INPUT);
    CONSOLE_READCONSOLE_CONTROL control = { sizeof(control), 0, 1u << 25, 0 };
    BOOL success = real_read(handle, header, ARRAYSIZE(header) - 1, &read, &control);
    SetConsoleMode(handle, mode);
    if (!success || !read || header[read - 1] != 25) return FALSE;
    header[read - 1] = 0;
    wchar_t *end;
    unsigned long lines = wcstoul(header, &end, 10);
    swprintf_s(expected, ARRAYSIZE(expected), L";%s", token);
    if (!lines || lines > 65536 || wcscmp(end, expected)) return FALSE;
    ai_remaining = lines;
    ai_armed = TRUE;
    ai_status = 0;
    swprintf_s(expected, ARRAYSIZE(expected), L"777;nauterm-command-ready=%s", token);
    osc(expected);
    return TRUE;
}

static BOOL WINAPI hooked_read(HANDLE handle, LPVOID buffer, DWORD capacity, LPDWORD read, PCONSOLE_READCONSOLE_CONTROL control) {
    DWORD mode;
    if (!GetConsoleMode(handle, &mode) || !(mode & ENABLE_LINE_INPUT))
        return real_read(handle, buffer, capacity, read, control);
    for (size_t i = 0; i < pending_length; ++i) visible(pending[i]);
    pending_length = 0;
    if (!capturing) {
        BOOL success = real_read(handle, buffer, capacity, read, control);
        if (success) consumed_ai_lines((const wchar_t *)buffer, *read);
        return success;
    }
    capturing = FALSE;
    if (active && !probing) {
        wchar_t injection[256];
        int length = swprintf_s(injection, ARRAYSIZE(injection), L"@echo __nauterm_exit_%s;%%errorlevel%%\r\n", token);
        if (length <= 0 || (DWORD)length >= capacity) { event_error(); }
        else {
            memcpy(buffer, injection, length * sizeof(wchar_t));
            *read = length;
            prompt_length = 0;
            probe_length = 0;
            probing = TRUE;
            return TRUE;
        }
    }
    if (probing) {
        probing = FALSE;
        complete();
    }
    prompt_start();
    write_wide(prompt, prompt_length);
    prompt_end();
    CONSOLE_READCONSOLE_CONTROL own_control = { sizeof(own_control), 0, 1u << 24, 0 };
    if (control) {
        own_control = *control;
        own_control.dwCtrlWakeupMask |= 1u << 24;
    }
    for (;;) {
        BOOL success = real_read(handle, buffer, capacity, read, &own_control);
        if (!success || !*read) { ai_armed = FALSE; return success; }
        wchar_t *line = (wchar_t *)buffer;
        if (line[*read - 1] == 24) {
            // Ctrl+X wakes the native cooked input reader. Discard its exact
            // pending buffer without submitting it, then acknowledge AI.
            own_control.nInitialChars = 0;
            emit(L"\r\n");
            prompt_start();
            write_wide(prompt, prompt_length);
            prompt_end();
            wchar_t payload[256];
            swprintf_s(payload, ARRAYSIZE(payload), L"777;nauterm-line-ready=%s", token);
            osc(payload);
            if (!read_ai_header(handle)) {
                event_error();
                return FALSE;
            }
            continue;
        }
        consumed_ai_lines(line, *read);
        begin(line, *read);
        if (!active && ai_armed && !ai_remaining) {
            wchar_t payload[256];
            swprintf_s(payload, ARRAYSIZE(payload), L"777;nauterm-command-end=%s;%d", token, ai_status);
            osc(payload);
            ai_armed = FALSE;
        }
        return success;
    }
}

static BOOL WINAPI hooked_setenv(LPCWSTR name, LPCWSTR value) {
    if (_wcsicmp(name, L"PROMPT")) return real_setenv(name, value);
    if (value && !wcsncmp(value, prefix, wcslen(prefix))) return real_setenv(name, value);
    if (!value || !*value) value = L"$P$G";
    size_t length = wcslen(prefix) + wcslen(value) + 1;
    wchar_t *prefixed = (wchar_t *)malloc(length * sizeof(wchar_t));
    if (!prefixed) return real_setenv(name, value);
    wcscpy_s(prefixed, length, prefix);
    wcscat_s(prefixed, length, value);
    BOOL result = real_setenv(name, prefixed);
    free(prefixed);
    return result;
}

static ULONG_PTR replacement(const char *name) {
    if (!strcmp(name, "ReadConsoleW")) return (ULONG_PTR)hooked_read;
    if (!strcmp(name, "WriteConsoleW")) return (ULONG_PTR)hooked_write;
    if (!strcmp(name, "WriteFile")) return (ULONG_PTR)hooked_file;
    if (!strcmp(name, "SetEnvironmentVariableW")) return (ULONG_PTR)hooked_setenv;
    return 0;
}

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID reserved) {
    (void)reserved;
    if (reason != DLL_PROCESS_ATTACH) return TRUE;
    DisableThreadLibraryCalls(instance);
    DWORD length = GetEnvironmentVariableW(L"NAUTERM_CMD_TOKEN", token, ARRAYSIZE(token));
    if (!length || length >= ARRAYSIZE(token)) return FALSE;
    for (DWORD i = 0; i < length; ++i) {
        wchar_t c = token[i];
        if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '-' || c == '_')) return FALSE;
    }
    HMODULE kernel = GetModuleHandleW(L"kernel32.dll");
    real_read = (ReadConsoleFn)(void *)GetProcAddress(kernel, "ReadConsoleW");
    real_write = (WriteConsoleFn)(void *)GetProcAddress(kernel, "WriteConsoleW");
    real_file = (WriteFileFn)(void *)GetProcAddress(kernel, "WriteFile");
    real_setenv = (SetEnvFn)(void *)GetProcAddress(kernel, "SetEnvironmentVariableW");
    if (!real_read || !real_write || !real_file || !real_setenv) return FALSE;
    swprintf_s(marker, ARRAYSIZE(marker), L"\x1b]777;nauterm-cmd-prompt=%s\x1b\\", token);
    swprintf_s(prefix, ARRAYSIZE(prefix), L"$E]777;nauterm-cmd-prompt=%s$E\\", token);
    marker_length = wcslen(marker);
    shell_pid = GetCurrentProcessId();
    output = GetStdHandle(STD_OUTPUT_HANDLE);
    // Patch imports only in this CMD image. There are no global detours, and
    // ReadConsole calls in subsequently launched programs remain untouched.
    BYTE *base = (BYTE *)GetModuleHandleW(NULL);
    IMAGE_DOS_HEADER *dos = (IMAGE_DOS_HEADER *)base;
    IMAGE_NT_HEADERS *nt = (IMAGE_NT_HEADERS *)(base + dos->e_lfanew);
    DWORD imports = nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT].VirtualAddress;
    BOOL patched_read = FALSE;
    for (IMAGE_IMPORT_DESCRIPTOR *descriptor = (IMAGE_IMPORT_DESCRIPTOR *)(base + imports); imports && descriptor->Name; ++descriptor) {
        if (!descriptor->OriginalFirstThunk) continue;
        IMAGE_THUNK_DATA *names = (IMAGE_THUNK_DATA *)(base + descriptor->OriginalFirstThunk);
        IMAGE_THUNK_DATA *slots = (IMAGE_THUNK_DATA *)(base + descriptor->FirstThunk);
        for (; names->u1.AddressOfData; ++names, ++slots) {
            if (IMAGE_SNAP_BY_ORDINAL(names->u1.Ordinal)) continue;
            IMAGE_IMPORT_BY_NAME *symbol = (IMAGE_IMPORT_BY_NAME *)(base + names->u1.AddressOfData);
            ULONG_PTR function = replacement((char *)symbol->Name);
            DWORD protection;
            if (function && VirtualProtect(&slots->u1.Function, sizeof(void *), PAGE_READWRITE, &protection)) {
                slots->u1.Function = function;
                VirtualProtect(&slots->u1.Function, sizeof(void *), protection, &protection);
                if (function == (ULONG_PTR)hooked_read) patched_read = TRUE;
            }
        }
    }
    if (!patched_read) return FALSE;
    wchar_t original[32768];
    length = GetEnvironmentVariableW(L"PROMPT", original, ARRAYSIZE(original));
    hooked_setenv(L"PROMPT", length && length < ARRAYSIZE(original) ? original : NULL);
    return TRUE;
}
#endif
