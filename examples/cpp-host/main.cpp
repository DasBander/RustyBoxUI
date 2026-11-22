#define WIN32_LEAN_AND_MEAN
#include "rustybox.hpp"
#include <filesystem>
#include <iostream>
#include <string_view>
#include <winrt/Windows.Data.Json.h>

namespace fs = std::filesystem;
using winrt::Windows::Data::Json::JsonObject;
using winrt::Windows::Data::Json::JsonValue;

LRESULT CALLBACK window_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto *view = reinterpret_cast<rustybox::Runtime *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    switch (message) {
    case WM_NCCALCSIZE:
        if (wparam)
            return 0;
        break;
    case WM_SIZE:
        if (view && LOWORD(lparam) && HIWORD(lparam)) {
            // Win32 callbacks must keep C++ exceptions on this side of the message pump.
            try {
                view->resize(LOWORD(lparam), HIWORD(lparam));
            } catch (const std::exception &) {
                PostMessageW(window, WM_CLOSE, 0, 0);
            }
        }
        return 0;
    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProcW(window, message, wparam, lparam);
}

void dispatch(rustybox::Runtime &view, const std::string &event_json) {
    auto event = JsonObject::Parse(winrt::to_hstring(event_json));
    if (event.GetNamedString(L"event", L"") != L"invoke")
        return;
    const auto request = event.GetNamedObject(L"request");
    JsonObject reply;
    reply.SetNamedValue(L"v", JsonValue::CreateNumberValue(1));
    // Both fields belong to the caller. A reload starts a different document session.
    reply.SetNamedValue(L"id", request.GetNamedValue(L"id"));
    reply.SetNamedValue(L"session", request.GetNamedValue(L"session"));
    const bool known = request.GetNamedString(L"method") == L"host.ping";
    reply.SetNamedValue(L"ok", JsonValue::CreateBooleanValue(known));
    if (known) {
        JsonObject result;
        result.SetNamedValue(L"pong", JsonValue::CreateBooleanValue(true));
        result.SetNamedValue(L"host", JsonValue::CreateStringValue(L"C++ embedded host"));
        reply.SetNamedValue(L"result", result);
    } else {
        JsonObject error;
        error.SetNamedValue(L"code", JsonValue::CreateStringValue(L"UNKNOWN_COMMAND"));
        error.SetNamedValue(L"message", JsonValue::CreateStringValue(L"Host command not registered"));
        reply.SetNamedValue(L"error", error);
    }
    view.post(winrt::to_string(reply.Stringify()));
}

int wmain(int argc, wchar_t **argv) {
    try {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        wchar_t path[32768]{};
        if (!GetModuleFileNameW(nullptr, path, 32768))
            throw std::runtime_error("Cannot find executable");
        const auto directory = fs::path(path).parent_path();
        auto assets = directory / L"ui";
        auto profile = directory / L"cpp-profile";
        bool hidden = false;
        for (int i = 1; i < argc; ++i) {
            const std::wstring_view option(argv[i]);
            if (option == L"--assets" && i + 1 < argc)
                assets = argv[++i];
            else if (option == L"--data-dir" && i + 1 < argc)
                profile = argv[++i];
            else if (option == L"--hidden")
                hidden = true;
        }

        rustybox::Runtime view((directory / L"rustybox_ui.dll").wstring());
        WNDCLASSW cls{};
        cls.style = CS_DBLCLKS;
        cls.lpfnWndProc = window_proc;
        cls.hInstance = GetModuleHandleW(nullptr);
        cls.lpszClassName = L"RustyBox.CppHost";
        if (!RegisterClassW(&cls))
            throw std::runtime_error("Cannot register C++ parent window");
        HWND parent =
            CreateWindowExW(0, cls.lpszClassName, L"RustyBox — C++ embedded host",
                            WS_OVERLAPPEDWINDOW & ~WS_CAPTION, CW_USEDEFAULT, CW_USEDEFAULT, 1440,
                            940, nullptr, nullptr, cls.hInstance, nullptr);
        if (!parent)
            throw std::runtime_error("Cannot create C++ parent window");

        const auto web = fs::absolute(assets).u8string();
        const auto data = fs::absolute(profile).u8string();
        const std::string title = "RustyBox C++ child view";
        RECT bounds{};
        GetClientRect(parent, &bounds);
        RBConfig config{};
        config.parent_window = reinterpret_cast<uintptr_t>(parent);
        config.width = bounds.right;
        config.height = bounds.bottom;
        config.flags = RB_RESERVED_ORIGIN | (hidden ? RB_HIDDEN : 0);
        config.assets = reinterpret_cast<const uint8_t *>(web.data());
        config.assets_len = web.size();
        config.data_dir = reinterpret_cast<const uint8_t *>(data.data());
        config.data_dir_len = data.size();
        config.title = reinterpret_cast<const uint8_t *>(title.data());
        config.title_len = title.size();
        view.open(config);
        SetWindowLongPtrW(parent, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(&view));
        if (!hidden)
            ShowWindow(parent, SW_SHOW);

        // This app owns the outer HWND and message pump. tick() services the child view.
        bool running = true;
        while (running && IsWindow(parent) && view.tick()) {
            MSG message{};
            while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
                if (message.message == WM_QUIT) {
                    running = false;
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            if (!running || !IsWindow(parent))
                break;
            std::string event;
            while (view.next_event(event))
                dispatch(view, event);
            Sleep(8);
        }
        if (IsWindow(parent))
            SetWindowLongPtrW(parent, GWLP_USERDATA, 0);
        view.close();
        if (IsWindow(parent))
            DestroyWindow(parent);
        UnregisterClassW(cls.lpszClassName, cls.hInstance);
        return 0;
    } catch (const winrt::hresult_error &error) {
        std::cerr << winrt::to_string(error.message()) << '\n';
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
    }
    return 1;
}
