#pragma once
#include "rustybox.h"
#include <windows.h>
#include <stdexcept>
#include <string>
#include <vector>
namespace rustybox {
// RAII wrapper for dynamic loading. Noncopyable and nonmovable: UI thread affinity is intentional.
class Runtime {
    HMODULE module_{};
    RBHandle handle_{};
    using Create = int32_t (*)(const RBConfig *, RBHandle *);
    using HandleCall = int32_t (*)(RBHandle);
    using Resize = int32_t (*)(RBHandle, uint32_t, uint32_t);
    using Post = int32_t (*)(RBHandle, const uint8_t *, size_t);
    using Event = int32_t (*)(RBHandle, uint8_t *, size_t, size_t *);
    using Error = size_t (*)(uint8_t *, size_t);
    Create create_{};
    HandleCall poll_{}, tick_{}, destroy_{};
    Resize resize_{};
    Post post_{};
    Event event_{};
    Error error_{};
    template <class T> T symbol(const char *name) {
        auto address = GetProcAddress(module_, name);
        if (!address)
            throw std::runtime_error(std::string("Missing SDK export: ") + name);
        return reinterpret_cast<T>(address);
    }
    void check(int32_t status) const {
        if (status < 0) {
            std::vector<uint8_t> buffer(error_(nullptr, 0) + 1);
            error_(buffer.data(), buffer.size());
            throw std::runtime_error(reinterpret_cast<const char *>(buffer.data()));
        }
    }

  public:
    explicit Runtime(const std::wstring &dll) {
        module_ =
            LoadLibraryExW(dll.c_str(), nullptr,
                           LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
        if (!module_)
            throw std::runtime_error("Cannot load rustybox_ui.dll");
        try {
            if (symbol<uint32_t (*)()>("rb_abi_version")() != RB_ABI_VERSION)
                throw std::runtime_error("RustyBox ABI mismatch");
            create_ = symbol<Create>("rb_create");
            poll_ = symbol<HandleCall>("rb_poll");
            tick_ = symbol<HandleCall>("rb_tick");
            destroy_ = symbol<HandleCall>("rb_destroy");
            resize_ = symbol<Resize>("rb_resize");
            post_ = symbol<Post>("rb_post_json");
            event_ = symbol<Event>("rb_next_event");
            error_ = symbol<Error>("rb_last_error");
        } catch (...) {
            FreeLibrary(module_);
            module_ = nullptr;
            throw;
        }
    }
    ~Runtime() {
        if (handle_)
            destroy_(handle_);
        if (module_)
            FreeLibrary(module_);
    }
    Runtime(const Runtime &) = delete;
    Runtime &operator=(const Runtime &) = delete;
    void open(RBConfig config) {
        if (handle_)
            throw std::runtime_error("Runtime already open");
        config.struct_size = sizeof(RBConfig);
        config.abi_version = RB_ABI_VERSION;
        check(create_(&config, &handle_));
    }
    bool poll() {
        auto status = poll_(handle_);
        check(status);
        return status != RB_CLOSED;
    }
    bool tick() {
        auto status = tick_(handle_);
        check(status);
        return status != RB_CLOSED;
    }
    void resize(uint32_t w, uint32_t h) {
        check(resize_(handle_, w, h));
    }
    void post(const std::string &json) {
        check(post_(handle_, reinterpret_cast<const uint8_t *>(json.data()), json.size()));
    }
    bool next_event(std::string &out) {
        size_t size = 0;
        auto status = event_(handle_, nullptr, 0, &size);
        if (status == RB_EMPTY)
            return false;
        if (status != RB_BUFFER_TOO_SMALL)
            check(status);
        std::vector<uint8_t> buffer(size);
        check(event_(handle_, buffer.data(), buffer.size(), &size));
        out.assign(buffer.begin(), buffer.begin() + size);
        return true;
    }
    void close() {
        if (handle_) {
            check(destroy_(handle_));
            handle_ = 0;
        }
    }
    RBHandle handle() const {
        return handle_;
    }
};
static_assert(sizeof(RBConfig) == 80, "This SDK build targets Windows x64");
} // namespace rustybox
