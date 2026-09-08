#include <stdint.h>
#include <stddef.h>

#if defined(__x86_64__) || defined(_M_X64)
#define WISP_MS_ABI __attribute__((ms_abi))
#else
#define WISP_MS_ABI
#endif

extern uint32_t wisp_impl_GetLastError(void);
extern void wisp_impl_SetLastError(uint32_t value);
extern void *wisp_impl_VirtualAlloc(void *address, size_t size, uint32_t type, uint32_t protection);
extern int32_t wisp_impl_VirtualProtect(void *address, size_t size, uint32_t protection, uint32_t *old_protection);
extern uint32_t wisp_impl_TlsAlloc(void);
extern int32_t wisp_impl_TlsFree(uint32_t index);
extern void *wisp_impl_TlsGetValue(uint32_t index);
extern int32_t wisp_impl_TlsSetValue(uint32_t index, void *value);
extern uintptr_t wisp_impl_CreateThread(uintptr_t start, void *parameter, uint32_t creation_flags, uint32_t *thread_id);
extern uint32_t wisp_impl_WaitForSingleObject(uintptr_t handle, uint32_t milliseconds);
extern int32_t wisp_impl_CloseHandle(uintptr_t handle);
extern uintptr_t wisp_impl_GetModuleHandleA(const char *name);
extern uintptr_t wisp_impl_GetModuleHandleW(const uint16_t *name);
extern uintptr_t wisp_impl_GetProcAddress(uintptr_t module, const char *name);
extern void wisp_impl_ExitProcess(uint32_t code);

WISP_MS_ABI uint32_t wisp_abi_GetLastError(void) { return wisp_impl_GetLastError(); }
WISP_MS_ABI void wisp_abi_SetLastError(uint32_t value) { wisp_impl_SetLastError(value); }
WISP_MS_ABI void *wisp_abi_VirtualAlloc(void *address, size_t size, uint32_t type, uint32_t protection) { return wisp_impl_VirtualAlloc(address, size, type, protection); }
WISP_MS_ABI int32_t wisp_abi_VirtualProtect(void *address, size_t size, uint32_t protection, uint32_t *old_protection) { return wisp_impl_VirtualProtect(address, size, protection, old_protection); }
WISP_MS_ABI uint32_t wisp_abi_TlsAlloc(void) { return wisp_impl_TlsAlloc(); }
WISP_MS_ABI int32_t wisp_abi_TlsFree(uint32_t index) { return wisp_impl_TlsFree(index); }
WISP_MS_ABI void *wisp_abi_TlsGetValue(uint32_t index) { return wisp_impl_TlsGetValue(index); }
WISP_MS_ABI int32_t wisp_abi_TlsSetValue(uint32_t index, void *value) { return wisp_impl_TlsSetValue(index, value); }
WISP_MS_ABI uintptr_t wisp_abi_CreateThread(void *security, size_t stack_size, uintptr_t start, void *parameter, uint32_t creation_flags, uint32_t *thread_id) {
    (void)security; (void)stack_size;
    return wisp_impl_CreateThread(start, parameter, creation_flags, thread_id);
}
WISP_MS_ABI uint32_t wisp_abi_WaitForSingleObject(uintptr_t handle, uint32_t milliseconds) { return wisp_impl_WaitForSingleObject(handle, milliseconds); }
WISP_MS_ABI int32_t wisp_abi_CloseHandle(uintptr_t handle) { return wisp_impl_CloseHandle(handle); }
WISP_MS_ABI uintptr_t wisp_abi_GetModuleHandleA(const char *name) { return wisp_impl_GetModuleHandleA(name); }
WISP_MS_ABI uintptr_t wisp_abi_GetModuleHandleW(const uint16_t *name) { return wisp_impl_GetModuleHandleW(name); }
WISP_MS_ABI uintptr_t wisp_abi_GetProcAddress(uintptr_t module, const char *name) { return wisp_impl_GetProcAddress(module, name); }
WISP_MS_ABI void wisp_abi_ExitProcess(uint32_t code) { wisp_impl_ExitProcess(code); }

uint32_t wisp_invoke_thread_start_msabi(uintptr_t start, void *parameter) {
    typedef uint32_t (WISP_MS_ABI *thread_proc_t)(void *);
    thread_proc_t proc = (thread_proc_t)start;
    return proc(parameter);
}
