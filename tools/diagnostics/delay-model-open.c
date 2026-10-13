// Diagnostic interposer only: delay installed-package GLB opens on loader threads.
// cc -shared -fPIC -o /tmp/delay-model-open.so delay-model-open.c -ldl
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <time.h>
static void delay(const char *path) {
    const char *ms = getenv("FREEFALL_TEST_MODEL_DELAY_MS");
    if (!ms || !strstr(path, "/models/") || !strstr(path, "/model.glb")) return;
    long n = strtol(ms, 0, 10);
    if (n <= 0 || n > 60000) return;
    fprintf(stderr, "test delayed model open %ldms: %s\n", n, path);
    struct timespec t = {n/1000,(n%1000)*1000000};
    while (nanosleep(&t,&t)) {}
}
int open64(const char *path, int flags, ...) {
    mode_t mode = 0;
    if ((flags & O_CREAT) || ((flags & O_TMPFILE) == O_TMPFILE)) {
        va_list args; va_start(args,flags); mode=va_arg(args,int); va_end(args);
    }
    delay(path);
    int (*real)(const char*,int,...) = dlsym(RTLD_NEXT,"open64");
    return real(path,flags,mode);
}
