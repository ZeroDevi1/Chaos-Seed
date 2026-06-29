#ifndef C_PRIVATE_PIP_H
#define C_PRIVATE_PIP_H

#import <AppKit/AppKit.h>
#import <stdbool.h>

typedef void (*ChaosPrivatePIPCallback)(void * _Nullable context);

bool ChaosPrivatePIPIsAvailable(void);

void * _Nullable ChaosPrivatePIPCreate(
    NSView * _Nonnull contentView,
    NSString * _Nullable title,
    NSSize aspectRatio,
    bool playing,
    NSWindow * _Nullable replacementWindow,
    NSRect replacementRect,
    ChaosPrivatePIPCallback _Nullable didClose,
    void * _Nullable context,
    NSString * _Nullable * _Nullable error
);

void ChaosPrivatePIPClose(void * _Nullable session);

bool ChaosPrivatePIPIsActive(void * _Nullable session);

void ChaosPrivatePIPSetPlaying(void * _Nullable session, bool playing);

void ChaosPrivatePIPDestroy(void * _Nullable session);

#endif
