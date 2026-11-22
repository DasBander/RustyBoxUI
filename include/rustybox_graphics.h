#ifndef RUSTYBOX_GRAPHICS_H
#define RUSTYBOX_GRAPHICS_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* Optional graphics DLL; same-thread handles. Bounds are physical pixels.
   ABI 1 presents D3D11 host textures and a built-in sample scene. */
__declspec(dllimport) uint32_t rbg_abi_version(void);
__declspec(dllimport) int32_t rbg_create(uintptr_t parent, uint32_t width, uint32_t height,
                                         uint64_t *out);
__declspec(dllimport) int32_t rbg_resize(uint64_t handle, int32_t x, int32_t y, uint32_t width,
                                         uint32_t height);
__declspec(dllimport) int32_t rbg_frame(uint64_t handle, float elapsed_seconds);
__declspec(dllimport) int32_t rbg_hardware(uint64_t handle);
__declspec(dllimport) int32_t rbg_destroy(uint64_t handle);
/* Visual is an IDCompositionVisual. The caller commits its composition device.
   Texture and device reference rules match the rb_graphics_* functions. */
__declspec(dllimport) int32_t rbg_create_visual(void *visual, uint32_t width, uint32_t height,
                                                uint64_t *out);
__declspec(dllimport) int32_t rbg_get_device(uint64_t handle, void **device);
__declspec(dllimport) int32_t rbg_set_texture(uint64_t handle, void *texture);
__declspec(dllimport) int32_t rbg_set_view(uint64_t handle, float zoom, uint32_t channel,
                                           uint32_t motion);
#ifdef __cplusplus
}
#endif
#endif
