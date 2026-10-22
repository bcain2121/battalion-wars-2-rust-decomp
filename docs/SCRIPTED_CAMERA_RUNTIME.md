# Scripted camera runtime — partial

Follow-up: `RADIO_AND_FADE_RUNTIME.md` records 152-test validation, actual original radio PCM, timed fade and angular-rate FOV support. The unmodified intro now reaches FollowWaypoint at instruction 177.

The original mission is still incomplete. No cinematic rendering or completed cutscene is claimed.

Implemented `mission_camera.rs`: loads all 10 authored cameras and 637 waypoints, including base FOV (radians converted to degrees), targets, successors, speeds and pause durations. Restricted `SetCamera`, `CameraSetWaypoint`, and `CameraSetTarget` update mission camera state. Active position and target paths advance transactionally with the mission clock; pause holds them. Zero-length cyclic traversal is bounded and failures preserve camera state. Unknown cameras/targets, branching paths and unsupported behavioral flags fail explicitly. Flag 0x20 is recognized as WP_CAMERA_WAYPOINT, using the already attributed editor enum reference.

**Linear waypoint motion and pause handling are replacement policies, not recovered camera spline/speed parity.** Chase-camera activation, FOV commands, fades/waits and presentation integration are unfinished. Pose validation rejects missing/coincident targets and nonfinite values.

An explicit original-intro prefix fixture executes instructions 0–92 followed by a synthetic return. It verifies the actual camera/waypoint/target assignments, finite moving poses, authored 45-degree FOV and scheduler pause. This is not a full intro, and no screenshot/human visual acceptance is claimed.

Configured isolated probes now stop at intro instruction **96, ClearMessageQueue**, rather than SetCamera:

```sh
cargo run --locked --bin bw-mission-audit -- --authored-startup
```

Validation: **145 tests** (108 viewer-library, 9 startup, 4 original-assets, 9 original-mission, 15 Python). Warning-denying Clippy, formatting and locked battle build pass. Existing native Fort Assault victory/restart/defeat regression passes. Logs: `local/analysis/camera-runtime-{unit-tests,original-tests,python-tests,clippy,build,battle-regression}.log`; probe data: `local/analysis/camera-configured-audit.json`.

Next integration dependencies remain dialogue queue semantics, camera fades/FOV, actual audio/visual consumption, remaining mission contracts and required gameplay. No launcher replacement, publication or commit.
