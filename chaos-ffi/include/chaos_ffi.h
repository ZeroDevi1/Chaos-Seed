#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Compatibility: keep the export macro symbol available to consumers.
// Note: the generated bindings currently do not use it in prototypes.
#ifdef _WIN32
#  ifdef CHAOS_FFI_BUILD
#    define CHAOS_FFI_EXPORT __declspec(dllexport)
#  else
#    define CHAOS_FFI_EXPORT __declspec(dllimport)
#  endif
#else
#  define CHAOS_FFI_EXPORT
#endif

// NOTE:
// - All returned `char*` are UTF-8 and must be freed by `chaos_ffi_string_free`.
// - This header is a stable wrapper; the function prototypes live in the generated bindings.

#include "chaos_ffi_bindings.h"

// Backward-compatible alias for older headers.
typedef ChaosDanmakuCallback chaos_danmaku_callback;

#ifdef __cplusplus
} // extern "C"
#endif
