#import "CPrivatePIP.h"
#import <dlfcn.h>

@class PIPMutablePlaybackState;

@protocol PIPViewControllerDelegate <NSObject>
@optional
- (void)pipWillClose:(id)pip;
- (void)pipDidClose:(id)pip;
- (void)pipActionPlay:(id)pip;
- (void)pipActionPause:(id)pip;
- (void)pipActionStop:(id)pip;
- (void)pipAction:(id)pip skipInterval:(NSTimeInterval)interval;
@end

@interface PIPViewController : NSViewController
@property (nonatomic, copy, nullable) NSString *name;
@property (nonatomic, weak, nullable) id<PIPViewControllerDelegate> delegate;
@property (nonatomic, weak, nullable) NSWindow *replacementWindow;
@property (nonatomic) NSRect replacementRect;
@property (nonatomic) bool playing;
@property (nonatomic) NSSize aspectRatio;
- (void)presentViewControllerAsPictureInPicture:(NSViewController *)viewController;
- (void)updatePlaybackStateUsingBlock:(void (NS_NOESCAPE ^)(PIPMutablePlaybackState *state))updateBlock;
@end

@interface PIPMutablePlaybackState : NSObject
@property (nonatomic) NSTimeInterval contentDuration;
@property (nonatomic) NSInteger contentType;
- (void)setPlaybackRate:(double)playbackRate
            elapsedTime:(NSTimeInterval)elapsedTime
      timeControlStatus:(NSInteger)timeControlStatus;
@end

@interface ChaosPrivatePIPSession : NSObject <PIPViewControllerDelegate, NSWindowDelegate>
@property (nonatomic, strong) PIPViewController *pip;
@property (nonatomic, strong) NSViewController *contentController;
@property (nonatomic, weak) NSView *contentView;
@property (nonatomic, weak) NSView *originalSuperview;
@property (nonatomic, weak) NSWindow *replacementWindow;
@property (nonatomic) NSRect replacementRect;
@property (nonatomic) NSRect originalFrame;
@property (nonatomic) NSAutoresizingMaskOptions originalAutoresizingMask;
@property (nonatomic) NSInteger originalSubviewIndex;
@property (nonatomic) bool active;
@property (nonatomic) bool restored;
@property (nonatomic) bool preparedForClosure;
@property (nonatomic) ChaosPrivatePIPCallback didClose;
@property (nonatomic) void *callbackContext;
@end

static void *ChaosPrivatePIPLoadFramework(void) {
    static void *handle = NULL;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        handle = dlopen(
            "/System/Library/PrivateFrameworks/PIP.framework/PIP",
            RTLD_NOW | RTLD_LOCAL
        );
    });
    return handle;
}

bool ChaosPrivatePIPIsAvailable(void) {
    if (!ChaosPrivatePIPLoadFramework()) {
        return false;
    }
    Class pipClass = NSClassFromString(@"PIPViewController");
    return pipClass != Nil
        && [pipClass instancesRespondToSelector:@selector(presentViewControllerAsPictureInPicture:)];
}

static void ChaosPrivatePIPSetError(NSString **error, NSString *message) {
    if (error != NULL) {
        *error = message;
    }
}

@implementation ChaosPrivatePIPSession

- (BOOL)startWithContentView:(NSView *)contentView
                       title:(NSString *)title
                 aspectRatio:(NSSize)aspectRatio
                     playing:(BOOL)playing
            replacementWindow:(NSWindow *)replacementWindow
              replacementRect:(NSRect)replacementRect
                       error:(NSString **)error {
    if (!ChaosPrivatePIPIsAvailable()) {
        ChaosPrivatePIPSetError(error, @"当前系统未提供私有 PIP.framework");
        return NO;
    }

    Class pipClass = NSClassFromString(@"PIPViewController");
    id pipObject = [[pipClass alloc] init];
    if (![pipObject isKindOfClass:[NSViewController class]]) {
        ChaosPrivatePIPSetError(error, @"无法创建私有 PiP 控制器");
        return NO;
    }

    self.contentView = contentView;
    self.originalSuperview = contentView.superview;
    self.replacementWindow = replacementWindow ?: contentView.window;
    self.replacementRect = replacementRect;
    self.originalFrame = contentView.frame;
    self.originalAutoresizingMask = contentView.autoresizingMask;
    self.originalSubviewIndex = NSNotFound;
    if (self.originalSuperview != nil) {
        self.originalSubviewIndex = [self.originalSuperview.subviews indexOfObject:contentView];
    }

    NSViewController *contentController = [[NSViewController alloc] init];
    contentController.view = contentView;

    self.pip = (PIPViewController *)pipObject;
    self.pip.delegate = self;
    self.pip.playing = playing;
    self.pip.name = title;
    if (aspectRatio.width > 0 && aspectRatio.height > 0) {
        self.pip.aspectRatio = aspectRatio;
    }
    self.contentController = contentController;

    @try {
        [self.pip presentViewControllerAsPictureInPicture:contentController];
        self.active = YES;
        self.restored = NO;
        self.preparedForClosure = NO;
        [self updatePlaybackStatePlaying:playing];
        return YES;
    } @catch (NSException *exception) {
        [self restoreContentView];
        ChaosPrivatePIPSetError(
            error,
            [NSString stringWithFormat:@"私有 PiP 启动失败：%@", exception.reason ?: exception.name]
        );
        return NO;
    }
}

- (void)close {
    if (!self.active) {
        [self restoreContentView];
        return;
    }
    @try {
        [self prepareForClosure];
        [self.pip dismissViewController:self.contentController];
    } @catch (__unused NSException *exception) {
        [self finishClosing];
    }
}

- (void)prepareForClosure {
    if (self.preparedForClosure) {
        return;
    }
    self.preparedForClosure = YES;
    if (self.replacementWindow != nil) {
        self.pip.replacementWindow = self.replacementWindow;
        if (self.replacementRect.size.width > 0 && self.replacementRect.size.height > 0) {
            self.pip.replacementRect = self.replacementRect;
        }
        [NSApp activateIgnoringOtherApps:YES];
        [self.replacementWindow deminiaturize:self.pip];
    }
}

- (void)finishClosing {
    if (!self.active && self.restored) {
        return;
    }
    self.active = NO;
    [self restoreContentView];
    if (self.didClose != NULL) {
        self.didClose(self.callbackContext);
    }
}

- (void)restoreContentView {
    if (self.restored) {
        return;
    }
    NSView *contentView = self.contentView;
    NSView *superview = self.originalSuperview;
    if (contentView != nil && superview != nil && contentView.superview != superview) {
        contentView.frame = self.originalFrame;
        contentView.autoresizingMask = self.originalAutoresizingMask;
        if (self.originalSubviewIndex != NSNotFound
            && self.originalSubviewIndex <= (NSInteger)superview.subviews.count) {
            [superview addSubview:contentView
                       positioned:NSWindowBelow
                       relativeTo:self.originalSubviewIndex < (NSInteger)superview.subviews.count
                                  ? superview.subviews[self.originalSubviewIndex]
                                  : nil];
        } else {
            [superview addSubview:contentView];
        }
        contentView.needsDisplay = YES;
    }
    self.restored = YES;
}

- (void)updatePlaybackStatePlaying:(BOOL)playing {
    self.pip.playing = playing;
    if (![self.pip respondsToSelector:@selector(updatePlaybackStateUsingBlock:)]) {
        return;
    }
    [self.pip updatePlaybackStateUsingBlock:^(PIPMutablePlaybackState *state) {
        state.contentType = 1;
        state.contentDuration = 0;
        [state setPlaybackRate:playing ? 1.0 : 0.0
                   elapsedTime:0
             timeControlStatus:playing ? 2 : 0];
    }];
}

- (void)pipWillClose:(id)pip {
    [self prepareForClosure];
}

- (void)pipDidClose:(id)pip {
    [self finishClosing];
}

- (void)pipActionPlay:(id)pip {
    [self updatePlaybackStatePlaying:YES];
}

- (void)pipActionPause:(id)pip {
    [self updatePlaybackStatePlaying:NO];
}

- (void)pipActionStop:(id)pip {
    [self close];
}

@end

void *ChaosPrivatePIPCreate(
    NSView *contentView,
    NSString *title,
    NSSize aspectRatio,
    bool playing,
    NSWindow *replacementWindow,
    NSRect replacementRect,
    ChaosPrivatePIPCallback didClose,
    void *context,
    NSString **error
) {
    if (contentView == nil) {
        ChaosPrivatePIPSetError(error, @"PiP 内容视图为空");
        return NULL;
    }
    ChaosPrivatePIPSession *session = [[ChaosPrivatePIPSession alloc] init];
    session.didClose = didClose;
    session.callbackContext = context;
    if (![session startWithContentView:contentView
                                 title:title
                           aspectRatio:aspectRatio
                               playing:playing
                     replacementWindow:replacementWindow
                       replacementRect:replacementRect
                                 error:error]) {
        return NULL;
    }
    return (__bridge_retained void *)session;
}

void ChaosPrivatePIPClose(void *session) {
    if (session == NULL) {
        return;
    }
    ChaosPrivatePIPSession *pipSession = (__bridge ChaosPrivatePIPSession *)session;
    [pipSession close];
}

bool ChaosPrivatePIPIsActive(void *session) {
    if (session == NULL) {
        return false;
    }
    ChaosPrivatePIPSession *pipSession = (__bridge ChaosPrivatePIPSession *)session;
    return pipSession.active;
}

void ChaosPrivatePIPSetPlaying(void *session, bool playing) {
    if (session == NULL) {
        return;
    }
    ChaosPrivatePIPSession *pipSession = (__bridge ChaosPrivatePIPSession *)session;
    [pipSession updatePlaybackStatePlaying:playing];
}

void ChaosPrivatePIPDestroy(void *session) {
    if (session == NULL) {
        return;
    }
    ChaosPrivatePIPSession *pipSession = (__bridge_transfer ChaosPrivatePIPSession *)session;
    [pipSession close];
}
