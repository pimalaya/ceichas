# The checks a CI runner cannot run on its own host: a NixOS guest, root
# inside, sets its clock and suspends over a time, on every Linux backend,
# through examples/at.rs.

{ pkgs, src }:

let
  at = pkgs.rustPlatform.buildRustPackage {
    pname = "ceichas-at";
    version = "0.1.0";
    inherit src;
    cargoLock.lockFile = src + "/Cargo.lock";
    cargoBuildFlags = [
      "--example"
      "at"
    ];
    doCheck = false;
    postInstall = ''
      install -Dm755 "$(find target -path '*/release/examples/at' -type f)" "$out/bin/ceichas-at"
    '';
  };

in
pkgs.testers.runNixOSTest {
  name = "ceichas";

  nodes.machine = {
    environment.systemPackages = [ at ];

    # NOTE: the test sets the clock by hand, nothing may set it back.
    services.timesyncd.enable = false;
  };

  testScript = ''
    import re

    BACKENDS = ["", "polling"]


    def answer(output):
        found = re.search(
            r"(Reached|Woken) backend=(\w+) answered_ms=(-?\d+) target_ms=(-?\d+) wall_ms=(-?\d+) mono_ms=(\d+)",
            output,
        )
        assert found, f"unexpected output {output!r}"
        wait, backend, *numbers = found.groups()
        return (wait, backend, *map(int, numbers))


    def now_ms():
        return int(machine.succeed("date +%s%3N"))


    def move_clock(seconds):
        """Moves the clock by `seconds`, answers the new time in ms."""
        return int(machine.succeed(f"date -s @$(( $(date +%s) + {seconds} )) > /dev/null; date +%s%3N"))


    machine.wait_for_unit("multi-user.target")

    with subtest("a datetime is reached at that datetime"):
        for backend in BACKENDS:
            target = now_ms() // 1000 + 3
            wait, name, answered, target_ms, _, _ = answer(machine.succeed(f"ceichas-at {target} {backend}"))
            assert wait == "Reached", f"{name} answered {wait}"
            late = answered - target_ms
            assert 0 <= late <= 1000, f"{name} answered {late} ms after its time"

    with subtest("a clock set past the time ends the wait"):
        for backend in BACKENDS:
            machine.succeed(f"ceichas-at +3600 {backend} > /tmp/forward 2>&1 &")
            machine.sleep(1)
            machine.fail("grep -q Reached /tmp/forward")
            moved = move_clock(7200)
            machine.wait_until_succeeds("grep -q Reached /tmp/forward", timeout=10)
            wait, name, answered, *_ = answer(machine.succeed("cat /tmp/forward"))
            assert answered - moved <= 1000, f"{name} answered {answered - moved} ms after the clock moved"
            move_clock(-7200)

    with subtest("a clock set back keeps the wait going"):
        for backend in BACKENDS:
            machine.succeed(f"ceichas-at +5 {backend} > /tmp/back 2>&1 &")
            move_clock(-60)
            machine.sleep(8)
            machine.fail("grep -q Reached /tmp/back")
            moved = move_clock(60)
            machine.wait_until_succeeds("grep -q Reached /tmp/back", timeout=10)
            wait, name, answered, *_ = answer(machine.succeed("cat /tmp/back"))
            assert answered - moved <= 1000, f"{name} answered {answered - moved} ms after the clock moved"

    with subtest("suspended time counts"):
        for backend in BACKENDS:
            machine.succeed(f"ceichas-at +10 {backend} > /tmp/suspend 2>&1 &")
            machine.succeed("(sleep 10; date +%s%3N) > /tmp/control 2>&1 &")
            start = now_ms()
            machine.succeed("rtcwake -m freeze -s 30")
            resumed = now_ms()
            machine.wait_until_succeeds("grep -q Reached /tmp/suspend", timeout=10)
            wait, name, answered, _, wall, mono = answer(machine.succeed("cat /tmp/suspend"))
            assert wall - mono >= 20000, f"the suspend left the monotonic clock running: wall {wall} ms, monotonic {mono} ms"
            assert answered - resumed <= 1000, f"{name} answered {answered - resumed} ms after the resume"
            machine.wait_until_succeeds("test -s /tmp/control", timeout=30)
            control = int(machine.succeed("cat /tmp/control"))
            late = control - start - 10000
            assert late >= 15000, f"the monotonic sleep ended {late} ms late only"
  '';
}
