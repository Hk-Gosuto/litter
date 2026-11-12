# iOS home startup behavior

The app always starts on the landing page with no selected computer and no
history list. Cached session summaries, the saved composer server preference,
the Rust active-thread key, and background reconnects do not select a computer
or navigate to a conversation.

Tapping a computer selects its chat list; tapping it again returns to the landing
page. A disconnected remembered computer can be selected while reconnecting.
Its cached history remains visible through transport changes and foreground
returns. Removing the selected computer explicitly returns to the landing page.
Search and the All Sessions action remain explicit ways to browse all history.

The visible pinned/recent projection is scoped before applying pin preferences,
so pins on one computer cannot hide another computer's history. The home
composer can still choose an available send target independently of the history
scope. Explicit notification/deep-link navigation continues to open its target;
the old automatic active-thread navigation on launch has been removed.

Animated home images honor the SwiftUI-proposed dimensions and clip decoded
contents. Decoding a full-size animation cannot change the initial layout.
The landing-page illustration explicitly uses its original 360-point width,
limited to the available width minus 32 points on narrow screens. This preserves
the old visual size while retaining stable bounds during async decoding.

Validation on iPhone 18 Pro / iOS 27.0: 22 native tests passed, covering a cold
launch with persisted scope and cached active thread, cache-to-live hydration,
explicit selection, reconnect/foreground return, per-computer pins, first-footer
layout, and animation dimensions before/after decoding. The final simulator
build and unsigned iPhoneOS Release archive passed. Build `210020004` contains
this behavior; the IPA includes the Watch companion and is checked for signing
metadata and ZIP integrity by the packaging script.
