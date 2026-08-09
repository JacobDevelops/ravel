# Synthetic review record

Result: PASS

The complete contract fixtures bind a review record by path, and the milestone gate recomputes that
record's digest the way it does for a real milestone. This file is what they bind, so the fixtures
exercise the binding rather than skipping it on an unreadable name. No review was performed; the
fixtures are a schema contract, not evidence.
