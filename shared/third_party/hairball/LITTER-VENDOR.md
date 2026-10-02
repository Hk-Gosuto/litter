# Hairball renderer snapshot

Upstream: https://github.com/dnakov/hairball
Revision: 42d52f5a5aa788b49b02f96c682949478cf01e91
The upstream snapshot contains no standalone LICENSE file; original source
headers and attribution are preserved.

Litter qualifies Hairball.Document in four HairballUI files because Xcode 27
adds SwiftUI.Document, making unqualified references ambiguous. The snapshot
makes this existing compatibility fix reproducible without modifying installed
SwiftPM checkouts. Other sources are unchanged. Compare with upstream when
refreshing the renderer and retire these qualifications when upstream adopts them.
