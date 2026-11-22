#ifndef RUSTYBOX_H
#define RUSTYBOX_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
#define RB_ABI_VERSION 1u
#if defined(_WIN32)
#define RB_API __declspec(dllimport)
#else
#define RB_API
#endif
typedef uint64_t RBHandle;
enum RBStatus {
    RB_OK = 0,
    RB_CLOSED = 1,
    RB_EMPTY = 2,
    RB_INVALID_ARGUMENT = -1,
    RB_INVALID_HANDLE = -2,
    RB_PLATFORM_ERROR = -3,
    RB_ABI_MISMATCH = -4,
    RB_BUFFER_TOO_SMALL = -5,
    RB_BUSY = -6
};
/* RB_RESERVED_ORIGIN maps assets to https://rustybox.example. Omit it to keep the existing
   https://rustybox.local browser storage origin. Native storage is shared within a profile. */
enum RBFlags {
    RB_DEBUG = 1,
    RB_HIDDEN = 2,
    RB_SELF_TEST = 4,
    RB_COMPOSED = 8,
    RB_RESERVED_ORIGIN = 16
};
#define RB_PENDING 3
typedef struct RBConfig {
    uint32_t struct_size;
    uint32_t abi_version;
    uintptr_t parent_window;
    uint32_t width, height, flags, reserved;
    const uint8_t *title;
    size_t title_len;
    const uint8_t *assets;
    size_t assets_len;
    const uint8_t *data_dir;
    size_t data_dir_len;
} RBConfig;
/* View calls must run on the creating STA thread. All UTF-8 inputs are copied.
   Pointers must be valid, aligned, and live for the duration of each call.
   rb_create pumps messages while WebView2 initializes; do not call it from a UI callback.
   Destroy all handles before releasing the DLL. Packaged WebView2 streams can outlive their windows
   and keep the UI module mapped until process exit. Do not call the SDK from DllMain. */
RB_API uint32_t rb_abi_version(void);
RB_API int32_t rb_create(const RBConfig *config, RBHandle *out);
/* Pump the creating STA between status calls. OK promotes the ticket to a view.
   RB_PENDING keeps it alive. Other statuses consume it. Timeout is 1..300000 ms. */
RB_API int32_t rb_create_begin(const RBConfig *config, uint32_t timeout_ms, RBHandle *ticket);
RB_API int32_t rb_create_status(RBHandle ticket);
RB_API int32_t rb_create_cancel(RBHandle ticket);
RB_API int32_t rb_collect_startups(void);
RB_API int32_t rb_wait(RBHandle view, uint32_t maximum_ms);
/* Device returns an owning ID3D11Device reference. Release on the view thread.
   Texture accepts a borrowed same-device, shader-readable single-sample RGBA8/BGRA8
   ID3D11Texture2D, or NULL to restore the sample. The DLL retains its own reference.
   Submit immediate-context writes on the view thread before the next tick. */
RB_API int32_t rb_graphics_device(RBHandle view, void **device);
RB_API int32_t rb_graphics_texture(RBHandle view, void *texture);
/* Each numbered viewport owns its D3D11 device. The original APIs address viewport zero. */
RB_API int32_t rb_graphics_device_at(RBHandle view, uint32_t viewport, void **device);
RB_API int32_t rb_graphics_texture_at(RBHandle view, uint32_t viewport, void *texture);
RB_API int32_t rb_graphics_reset(RBHandle view);
RB_API int32_t rb_poll(RBHandle handle);
RB_API int32_t rb_tick(RBHandle handle);
RB_API int32_t rb_resize(RBHandle handle, uint32_t width, uint32_t height);
RB_API uintptr_t rb_native_handle(RBHandle handle);
RB_API int32_t rb_post_json(RBHandle handle, const uint8_t *json, size_t length);
/* UTF-8 JSON without NUL. A size query returns RB_BUFFER_TOO_SMALL and does not consume the event.
 */
RB_API int32_t rb_next_event(RBHandle handle, uint8_t *buffer, size_t capacity, size_t *written);
RB_API int32_t rb_destroy(RBHandle handle);
/* Required byte length excluding NUL. If capacity > 0, copies a NUL-terminated message. */
RB_API size_t rb_last_error(uint8_t *buffer, size_t capacity);
#ifdef __cplusplus
}
#endif
#endif
