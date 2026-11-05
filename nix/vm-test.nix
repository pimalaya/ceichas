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

  # NOTE: the check runs as a unit of its own and reports on the kernel log,
  # so it does not depend on the backdoor shell across the suspends.
  suspend = pkgs.writeShellScript "ceichas-suspend" ''
    PATH=${
      pkgs.lib.makeBinPath [
        at
        pkgs.coreutils
        pkgs.util-linux
      ]
    }

    for backend in "" polling; do
      ceichas-at +10 $backend > /tmp/suspend 2>&1 &
      waiting=$!
      (sleep 10; date +%s%3N) > /tmp/control &
      control=$!
      start=$(date +%s%3N)
      rtcwake -m mem -s 30 > /dev/null
      resumed=$(date +%s%3N)
      wait $waiting $control
      echo "ceichas-suspend start=$start resumed=$resumed control=$(cat /tmp/control) $(cat /tmp/suspend)" > /dev/kmsg
    done
  '';

in
pkgs.testers.runNixOSTest {
  name = "ceichas";

  # NOTE: a guest that never answers fails the test instead of hanging it.
  globalTimeout = 900;

  nodes.machine = {
    environment.systemPackages = [ at ];

    # NOTE: the test sets the clock by hand, nothing may set it back.
    services.timesyncd.enable = false;

    # NOTE: suspend to RAM, which QEMU wakes on the RTC alarm; the guest does
    # not wake from suspend to idle. The store sits on a disk image, since
    # the 9p share of the host's store does not survive a suspend.
    boot.kernelParams = [ "mem_sleep_default=deep" ];
    virtualisation.qemu.options = [ "-global PIIX4_PM.disable_s3=0" ];
    virtualisation.useNixStoreImage = true;
  };

  testScript = ''
    import re
    import time

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
        modes = machine.succeed("cat /sys/power/mem_sleep")
        assert "[deep]" in modes, f"no suspend to RAM: {modes!r}"

        machine.succeed("systemd-run --unit=ceichas-suspend ${suspend}")

        # NOTE: `wait_for_console_text` reads one console line per second,
        # far behind the suspend logs, so the full log is polled instead.
        report = r"ceichas-suspend start=(\d+) resumed=(\d+) control=(\d+) (.*mono_ms=\d+)"
        deadline = time.monotonic() + 180

        while len(reports := list(re.finditer(report, machine.get_console_log()))) < len(BACKENDS):
            assert time.monotonic() < deadline, f"{len(reports)} reports for {len(BACKENDS)} backends"
            time.sleep(1)

        for found in reports:
            start, resumed, control = map(int, found.groups()[:3])
            wait, name, answered, _, wall, mono = answer(found.group(4))
            assert wait == "Reached", f"{name} answered {wait}"
            assert wall - mono >= 20000, f"the suspend left the monotonic clock running: wall {wall} ms, monotonic {mono} ms"
            assert answered - resumed <= 1000, f"{name} answered {answered - resumed} ms after the resume"
            late = control - start - 10000
            assert late >= 15000, f"the monotonic sleep ended {late} ms late only"
  '';
}
