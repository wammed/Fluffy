#!/usr/bin/env python3
"""
Automated Analyzer for Fluffy Switch Benchmarks
Aggregates structured events by switch_id and outputs detailed stage breakdown tables and CSVs.
Includes offline self-test functionality (--test).
"""

import sys
import re
import csv
import io
from collections import defaultdict

def parse_log(log_path_or_file):
    events_by_switch = defaultdict(list)
    unassociated_events = []
    unparseable_lines = 0

    # Regex to extract key="value" or key=value pairs
    kv_pattern = re.compile(r'([a-zA-Z0-9_]+)=(?:"([^"]*)"|([^\s]+))')

    if isinstance(log_path_or_file, str):
        f = open(log_path_or_file, 'r', encoding='utf-8', errors='replace')
        should_close = True
    else:
        f = log_path_or_file
        should_close = False

    try:
        for line in f:
            stripped = line.strip()
            if not stripped:
                continue

            if 'event=' not in stripped:
                unparseable_lines += 1
                continue

            matches = kv_pattern.findall(line)
            if not matches:
                unparseable_lines += 1
                continue

            data = {}
            for k, v_quoted, v_unquoted in matches:
                data[k] = v_quoted if v_quoted else v_unquoted

            event_name = data.get('event')
            if not event_name:
                unparseable_lines += 1
                continue

            switch_id = data.get('switch_id')
            if not switch_id:
                sid = data.get('session_id')
                out = data.get('output')
                gen = data.get('generation')
                if sid and out and gen:
                    switch_id = f"{sid}-{out}-gen{gen}"

            if switch_id:
                events_by_switch[switch_id].append(data)
            else:
                unassociated_events.append(data)
    finally:
        if should_close:
            f.close()

    return events_by_switch, unassociated_events, unparseable_lines

def analyze_switches(events_by_switch, output_csv=None):
    results = []
    validation_warnings = []

    for switch_id, events in sorted(events_by_switch.items()):
        ev_map = {}
        gst_events = []

        # Check duplicates and time ordering
        seen_events = set()
        last_total_ms = -1.0

        for ev in events:
            ev_name = ev.get('event', '')
            if ev_name.startswith('gst_sync_'):
                gst_events.append(ev)
            else:
                if ev_name in seen_events:
                    validation_warnings.append(f"Switch '{switch_id}' duplicate event: {ev_name}")
                seen_events.add(ev_name)
                ev_map[ev_name] = ev

            # Time monotonicity check
            tot_str = ev.get('total_elapsed_ms')
            if tot_str is not None:
                try:
                    tot_val = float(tot_str)
                    if last_total_ms >= 0.0 and tot_val < (last_total_ms - 0.5):
                        validation_warnings.append(
                            f"Switch '{switch_id}' time reversal: {ev_name} ({tot_val}ms < {last_total_ms}ms)"
                        )
                    last_total_ms = max(last_total_ms, tot_val)
                except ValueError:
                    validation_warnings.append(
                        f"Switch '{switch_id}' invalid total_elapsed_ms in {ev_name}: {tot_str}"
                    )

        output = events[0].get('output', 'unknown')
        gen = events[0].get('generation', '0')
        session_id = events[0].get('session_id', 'unknown')

        def get_ms(ev_dict, key='stage_elapsed_ms'):
            if not ev_dict:
                return 0.0
            val = ev_dict.get(key)
            if val is None and key == 'stage_elapsed_ms':
                val = ev_dict.get('elapsed_ms')
            try:
                return float(val) if val is not None else 0.0
            except ValueError:
                validation_warnings.append(f"Switch '{switch_id}' cannot parse {key}: {val}")
                return 0.0

        # Stage timings
        create_ms = get_ms(ev_map.get('new_pipeline_preroll_started'))
        set_paused_ms = get_ms(ev_map.get('pipeline_set_paused_returned'))
        preroll_wait_ms = get_ms(ev_map.get('preroll_wait_returned'))
        preroll_res = ev_map.get('preroll_wait_returned', {}).get('state_change_res', 'Unknown')

        async_done_ms = None
        sink_paused_ms = None
        pipe_paused_ms = None
        element_paused = []

        for gev in gst_events:
            gev_name = gev.get('event')
            src = gev.get('src', '')
            stage_el = get_ms(gev)

            if gev_name == 'gst_sync_async_done' and src == 'pipeline':
                async_done_ms = stage_el
            elif gev_name == 'gst_sync_state_changed':
                curr = gev.get('current_state')
                if src == 'waylandsink' and curr == 'Paused':
                    sink_paused_ms = stage_el
                elif src == 'pipeline' and curr == 'Paused':
                    pipe_paused_ms = stage_el
                elif curr == 'Paused':
                    element_paused.append((src, stage_el))

        set_playing_ms = get_ms(ev_map.get('pipeline_set_playing_returned'))
        old_teardown_ms = get_ms(ev_map.get('old_pipeline_set_null_returned'))

        # Support both new old_pipeline_rust_drop_completed and old old_pipeline_resources_released
        old_drop_ms = get_ms(ev_map.get('old_pipeline_rust_drop_completed'))
        if old_drop_ms == 0.0:
            old_drop_ms = get_ms(ev_map.get('old_pipeline_resources_released'))

        total_ms = get_ms(ev_map.get('playback_started'), key='total_elapsed_ms')
        if total_ms == 0.0:
            total_ms = get_ms(ev_map.get('playback_started'))

        # Validation & Status check
        has_failed = 'preroll_failed' in ev_map or ev_map.get('video_switch_completed', {}).get('success') == 'false'
        is_complete = 'playback_started' in ev_map

        if has_failed:
            status = 'FAILED'
        elif is_complete and 'Ok(Success)' in preroll_res:
            status = 'SUCCESS'
        elif is_complete:
            status = f'WARN({preroll_res})'
        else:
            status = 'INCOMPLETE'
            validation_warnings.append(f"Switch '{switch_id}' missing playback_started")

        if 'preroll_wait_returned' not in ev_map and not has_failed:
            validation_warnings.append(f"Switch '{switch_id}' missing preroll_wait_returned")

        results.append({
            'switch_id': switch_id,
            'output': output,
            'generation': gen,
            'session_id': session_id,
            'status': status,
            'create_ms': create_ms,
            'set_paused_ms': set_paused_ms,
            'sink_paused_ms': sink_paused_ms if sink_paused_ms is not None else 0.0,
            'gst_async_done_ms': async_done_ms if async_done_ms is not None else 0.0,
            'preroll_wait_ms': preroll_wait_ms,
            'preroll_res': preroll_res,
            'set_playing_ms': set_playing_ms,
            'old_teardown_ms': old_teardown_ms,
            'old_drop_ms': old_drop_ms,
            'total_switch_ms': total_ms,
            'elements_paused': element_paused,
        })

    if output_csv and results:
        fieldnames = [
            'switch_id', 'output', 'generation', 'session_id', 'status',
            'create_ms', 'set_paused_ms', 'sink_paused_ms', 'gst_async_done_ms',
            'preroll_wait_ms', 'preroll_res', 'set_playing_ms',
            'old_teardown_ms', 'old_drop_ms', 'total_switch_ms'
        ]
        with open(output_csv, 'w', newline='', encoding='utf-8') as f:
            writer = csv.DictWriter(f, fieldnames=fieldnames, extrasaction='ignore')
            writer.writeheader()
            for r in results:
                writer.writerow(r)
        print(f"Summary CSV exported to: {output_csv}")

    return results, validation_warnings

def print_summary_table(results, unassociated_events, unparseable_lines, validation_warnings):
    if not results:
        print("No switch events found in log.")
        return

    print("\n=== Fluffy Integrated Video Switch Benchmark Summary ===")
    header = (
        f"{'Switch ID':<32} | {'Status':<10} | {'Create':>7} | {'PrerollWait':>11} | {'SinkPaused':>10} | "
        f"{'AsyncDone':>9} | {'Playing':>7} | {'OldDrop':>8} | {'TotalLatency':>12}"
    )
    print(header)
    print("-" * len(header))

    for r in results:
        print(
            f"{r['switch_id']:<32} | "
            f"{r['status']:<10} | "
            f"{r['create_ms']:>6.2f}m | "
            f"{r['preroll_wait_ms']:>10.2f}m | "
            f"{r['sink_paused_ms']:>9.2f}m | "
            f"{r['gst_async_done_ms']:>8.2f}m | "
            f"{r['set_playing_ms']:>6.2f}m | "
            f"{r['old_drop_ms']:>7.2f}m | "
            f"{r['total_switch_ms']:>11.2f}m"
        )
    print("-" * len(header))
    print(f"Total Switches Analyzed : {len(results)}")
    print(f"Unassociated Event Logs : {len(unassociated_events)}")
    print(f"Unparseable Non-event   : {unparseable_lines} lines")

    if validation_warnings:
        print("\n[VALIDATION WARNINGS]:")
        for w in validation_warnings:
            print(f"  - {w}")
    else:
        print("Data Validation Check   : All switch sequences consistent (PASSED)")

def run_offline_tests():
    print("Running offline analyzer self-tests...")

    # Case 1: Standard successful switch with full GStreamer events and CSV verification
    synthetic_log_success = """
2026-10-10T00:00:01.000+09:00 INFO [OutputManager] Video switch requested event="video_switch_requested" switch_id="test-DP1-gen2" output="DP-1" generation=2 video_id="vidA" session_id="test" epoch_ms=1000
2026-10-10T00:00:01.002+09:00 INFO [Player] New pipeline created for video switch event="new_pipeline_created" switch_id="test-DP1-gen2" output="DP-1" generation=2 video_id="vidA" session_id="test" epoch_ms=1002 total_elapsed_ms=2.0
2026-10-10T00:00:01.010+09:00 INFO [Player] New pipeline preroll started event="new_pipeline_preroll_started" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1010 stage_elapsed_ms=8.0 total_elapsed_ms=10.0
2026-10-10T00:00:01.011+09:00 INFO [Player] Setting new pipeline state to PAUSED started event="pipeline_set_paused_started" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1011 total_elapsed_ms=11.0
2026-10-10T00:00:01.012+09:00 INFO [Player] Setting new pipeline state to PAUSED returned event="pipeline_set_paused_returned" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1012 stage_elapsed_ms=1.0 total_elapsed_ms=12.0
2026-10-10T00:00:01.012+09:00 INFO [Player] Waiting for new pipeline preroll started event="preroll_wait_started" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1012 total_elapsed_ms=12.0
2026-10-10T00:00:01.170+09:00 INFO [Player] Waiting for new pipeline preroll returned event="preroll_wait_returned" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1170 stage_elapsed_ms=158.0 total_elapsed_ms=170.0 state_change_res=Ok(Success) current_st=Paused pending_st=VoidPending
2026-10-10T00:00:01.170+09:00 INFO [Player] GStreamer sync event: STATE_CHANGED event="gst_sync_state_changed" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" stage_elapsed_ms=157.5 total_elapsed_ms=169.5 src="waylandsink" old_state=Ready current_state=Paused pending_state=VoidPending
2026-10-10T00:00:01.170+09:00 INFO [Player] GStreamer sync event: ASYNC_DONE event="gst_sync_async_done" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" stage_elapsed_ms=157.8 total_elapsed_ms=169.8 src="pipeline"
2026-10-10T00:00:01.171+09:00 INFO [Player] New pipeline first frame displayable event="new_pipeline_displayable" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1171 total_elapsed_ms=171.0
2026-10-10T00:00:01.172+09:00 INFO [Player] Setting new pipeline state to PLAYING started event="pipeline_set_playing_started" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1172 total_elapsed_ms=172.0
2026-10-10T00:00:01.173+09:00 INFO [Player] Setting new pipeline state to PLAYING returned event="pipeline_set_playing_returned" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1173 stage_elapsed_ms=1.0 total_elapsed_ms=173.0
2026-10-10T00:00:01.174+09:00 INFO [Player] Video switch committed event="video_switch_committed" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1174 total_elapsed_ms=174.0
2026-10-10T00:00:01.175+09:00 INFO [Player] Old pipeline teardown started event="old_pipeline_teardown_started" switch_id="test-DP1-gen2" output="DP-1" generation=1 session_id="test" epoch_ms=1175 total_elapsed_ms=175.0
2026-10-10T00:00:01.195+09:00 INFO [Player] Old pipeline set_state(NULL) returned event="old_pipeline_set_null_returned" switch_id="test-DP1-gen2" output="DP-1" generation=1 session_id="test" epoch_ms=1195 stage_elapsed_ms=20.0 total_elapsed_ms=195.0
2026-10-10T00:00:01.196+09:00 INFO [Player] Old pipeline teardown completed event="old_pipeline_teardown_completed" switch_id="test-DP1-gen2" output="DP-1" generation=1 session_id="test" epoch_ms=1196 stage_elapsed_ms=21.0 total_elapsed_ms=196.0
2026-10-10T00:00:01.276+09:00 INFO [Player] Old pipeline Rust handle drop completed event="old_pipeline_rust_drop_completed" switch_id="test-DP1-gen2" output="DP-1" generation=1 session_id="test" epoch_ms=1276 stage_elapsed_ms=80.0 total_elapsed_ms=276.0
2026-10-10T00:00:01.277+09:00 INFO [Player] Playback started after video switch event="playback_started" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1277 total_elapsed_ms=277.0
2026-10-10T00:00:01.278+09:00 INFO [OutputManager] Video switch completed successfully event="video_switch_completed" switch_id="test-DP1-gen2" output="DP-1" generation=2 session_id="test" epoch_ms=1278 success=true
"""
    f = io.StringIO(synthetic_log_success)
    events_by_switch, unassociated, unparseable = parse_log(f)
    results, warnings = analyze_switches(events_by_switch)
    assert len(results) == 1
    res = results[0]
    assert res['switch_id'] == "test-DP1-gen2"
    assert res['status'] == "SUCCESS"
    assert res['create_ms'] == 8.0
    assert res['preroll_wait_ms'] == 158.0
    assert res['sink_paused_ms'] == 157.5
    assert res['gst_async_done_ms'] == 157.8
    assert res['old_drop_ms'] == 80.0
    assert res['total_switch_ms'] == 277.0
    assert len(warnings) == 0

    # Verify CSV export and recalculation consistency
    out_csv = io.StringIO()
    fieldnames = [
        'switch_id', 'output', 'generation', 'session_id', 'status',
        'create_ms', 'set_paused_ms', 'sink_paused_ms', 'gst_async_done_ms',
        'preroll_wait_ms', 'preroll_res', 'set_playing_ms',
        'old_teardown_ms', 'old_drop_ms', 'total_switch_ms'
    ]
    writer = csv.DictWriter(out_csv, fieldnames=fieldnames, extrasaction='ignore')
    writer.writeheader()
    for r in results:
        writer.writerow(r)
    csv_str = out_csv.getvalue()
    assert "test-DP1-gen2,DP-1,2,test,SUCCESS,8.0,1.0,157.5,157.8,158.0,Ok(Success),1.0,20.0,80.0,277.0" in csv_str

    # Case 2: Multi-output and multi-generation separation
    synthetic_multi = """
2026-10-10T00:00:01.000+09:00 INFO [OutputManager] event="video_switch_requested" switch_id="test-DP1-gen2" output="DP-1" generation=2 video_id="vidA" session_id="test"
2026-10-10T00:00:01.000+09:00 INFO [OutputManager] event="video_switch_requested" switch_id="test-HDMI1-gen2" output="HDMI-A-1" generation=2 video_id="vidA" session_id="test"
2026-10-10T00:00:01.100+09:00 INFO [Player] event="preroll_wait_returned" switch_id="test-DP1-gen2" output="DP-1" generation=2 stage_elapsed_ms=100.0 total_elapsed_ms=100.0 state_change_res=Ok(Success)
2026-10-10T00:00:01.120+09:00 INFO [Player] event="playback_started" switch_id="test-DP1-gen2" output="DP-1" generation=2 total_elapsed_ms=120.0
2026-10-10T00:00:01.200+09:00 INFO [Player] event="preroll_wait_returned" switch_id="test-HDMI1-gen2" output="HDMI-A-1" generation=2 stage_elapsed_ms=200.0 total_elapsed_ms=200.0 state_change_res=Ok(Success)
2026-10-10T00:00:01.230+09:00 INFO [Player] event="playback_started" switch_id="test-HDMI1-gen2" output="HDMI-A-1" generation=2 total_elapsed_ms=230.0
"""
    f = io.StringIO(synthetic_multi)
    events_by_switch, _, _ = parse_log(f)
    results, warnings = analyze_switches(events_by_switch)
    assert len(results) == 2
    assert results[0]['switch_id'] == "test-DP1-gen2" and results[0]['total_switch_ms'] == 120.0
    assert results[1]['switch_id'] == "test-HDMI1-gen2" and results[1]['total_switch_ms'] == 230.0

    # Case 3: Failed switch detection
    synthetic_failed = """
2026-10-10T00:00:01.000+09:00 INFO [OutputManager] event="video_switch_requested" switch_id="test-DP1-gen3" output="DP-1" generation=3 session_id="test"
2026-10-10T00:00:01.050+09:00 ERROR [Player] event="preroll_failed" switch_id="test-DP1-gen3" output="DP-1" generation=3 total_elapsed_ms=50.0
2026-10-10T00:00:01.055+09:00 WARN [OutputManager] event="video_switch_completed" switch_id="test-DP1-gen3" output="DP-1" generation=3 success=false
"""
    f = io.StringIO(synthetic_failed)
    events_by_switch, _, _ = parse_log(f)
    results, _ = analyze_switches(events_by_switch)
    assert len(results) == 1
    assert results[0]['status'] == "FAILED"

    # Case 4: Incomplete switch (missing playback_started)
    synthetic_incomplete = """
2026-10-10T00:00:01.000+09:00 INFO [OutputManager] event="video_switch_requested" switch_id="test-DP1-gen4" output="DP-1" generation=4 session_id="test"
2026-10-10T00:00:01.100+09:00 INFO [Player] event="preroll_wait_returned" switch_id="test-DP1-gen4" output="DP-1" generation=4 stage_elapsed_ms=100.0 total_elapsed_ms=100.0 state_change_res=Ok(Success)
"""
    f = io.StringIO(synthetic_incomplete)
    events_by_switch, _, _ = parse_log(f)
    results, warnings = analyze_switches(events_by_switch)
    assert len(results) == 1
    assert results[0]['status'] == "INCOMPLETE"
    assert any("missing playback_started" in w for w in warnings)

    # Case 5: Time inversion & duplicate event warning detection
    synthetic_anomaly = """
2026-10-10T00:00:01.000+09:00 INFO [OutputManager] event="video_switch_requested" switch_id="test-DP1-gen5" output="DP-1" generation=5 session_id="test" total_elapsed_ms=100.0
2026-10-10T00:00:01.001+09:00 INFO [Player] event="new_pipeline_created" switch_id="test-DP1-gen5" output="DP-1" generation=5 session_id="test" total_elapsed_ms=50.0
2026-10-10T00:00:01.002+09:00 INFO [Player] event="new_pipeline_created" switch_id="test-DP1-gen5" output="DP-1" generation=5 session_id="test" total_elapsed_ms=150.0
2026-10-10T00:00:01.003+09:00 INFO [Player] event="playback_started" switch_id="test-DP1-gen5" output="DP-1" generation=5 session_id="test" total_elapsed_ms=200.0
"""
    f = io.StringIO(synthetic_anomaly)
    events_by_switch, _, _ = parse_log(f)
    results, warnings = analyze_switches(events_by_switch)
    assert any("time reversal" in w for w in warnings)
    assert any("duplicate event" in w for w in warnings)

    # Case 6: Unparseable lines and unassociated events counting
    synthetic_noise = """
Regular daemon log line with no event
another plain line
2026-10-10T00:00:01.000+09:00 INFO [System] event="daemon_started" pid=1234
2026-10-10T00:00:01.000+09:00 INFO [OutputManager] event="video_switch_requested" switch_id="test-DP1-gen6" output="DP-1" generation=6 session_id="test"
2026-10-10T00:00:01.100+09:00 INFO [Player] event="preroll_wait_returned" switch_id="test-DP1-gen6" output="DP-1" generation=6 stage_elapsed_ms=100.0 state_change_res=Ok(Success)
2026-10-10T00:00:01.120+09:00 INFO [Player] event="playback_started" switch_id="test-DP1-gen6" output="DP-1" generation=6 total_elapsed_ms=120.0
"""
    f = io.StringIO(synthetic_noise)
    events_by_switch, unassociated, unparseable = parse_log(f)
    assert unparseable == 2
    assert len(unassociated) == 1
    assert unassociated[0]['event'] == "daemon_started"

    # Case 7: Backward compatibility with legacy drop event name (old_pipeline_resources_released)
    synthetic_legacy = """
2026-10-10T00:00:01.000+09:00 INFO [OutputManager] event="video_switch_requested" switch_id="test-DP1-gen7" output="DP-1" generation=7 session_id="test"
2026-10-10T00:00:01.100+09:00 INFO [Player] event="preroll_wait_returned" switch_id="test-DP1-gen7" output="DP-1" generation=7 stage_elapsed_ms=100.0 state_change_res=Ok(Success)
2026-10-10T00:00:01.150+09:00 INFO [Player] event="old_pipeline_resources_released" switch_id="test-DP1-gen7" output="DP-1" generation=7 stage_elapsed_ms=75.0
2026-10-10T00:00:01.200+09:00 INFO [Player] event="playback_started" switch_id="test-DP1-gen7" output="DP-1" generation=7 total_elapsed_ms=200.0
"""
    f = io.StringIO(synthetic_legacy)
    events_by_switch, _, _ = parse_log(f)
    results, _ = analyze_switches(events_by_switch)
    assert len(results) == 1
    assert results[0]['old_drop_ms'] == 75.0

    print("All 7 offline self-test suites PASSED successfully!")

def main():
    if '--test' in sys.argv:
        run_offline_tests()
        sys.exit(0)

    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <FLUFFY_LOG_PATH> [OUTPUT_CSV_PATH]")
        print(f"       {sys.argv[0]} --test")
        sys.exit(1)

    log_path = sys.argv[1]
    csv_path = sys.argv[2] if len(sys.argv) > 2 else None

    events_by_switch, unassociated, unparseable = parse_log(log_path)
    results, warnings = analyze_switches(events_by_switch, output_csv=csv_path)
    print_summary_table(results, unassociated, unparseable, warnings)

if __name__ == '__main__':
    main()
