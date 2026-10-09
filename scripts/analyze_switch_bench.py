#!/usr/bin/env python3
"""
Automated Analyzer for Fluffy Switch Benchmarks
Aggregates structured events by switch_id and outputs detailed stage breakdown tables and CSVs.
"""

import sys
import re
import csv
from collections import defaultdict
from pathlib import Path

def parse_log(log_path):
    events_by_switch = defaultdict(list)
    unassociated_events = []

    # Regex to extract key="value" or key=value pairs
    kv_pattern = re.compile(r'([a-zA-Z0-9_]+)=(?:"([^"]*)"|([^\s]+))')

    with open(log_path, 'r', encoding='utf-8', errors='replace') as f:
        for line in f:
            if 'event=' not in line:
                continue

            matches = kv_pattern.findall(line)
            data = {}
            for k, v_quoted, v_unquoted in matches:
                data[k] = v_quoted if v_quoted else v_unquoted

            event_name = data.get('event')
            if not event_name:
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

    return events_by_switch, unassociated_events

def analyze_switches(events_by_switch, output_csv=None):
    results = []

    for switch_id, events in sorted(events_by_switch.items()):
        # Map event name to event record
        ev_map = {}
        gst_events = []

        for ev in events:
            ev_name = ev.get('event')
            if ev_name.startswith('gst_sync_'):
                gst_events.append(ev)
            else:
                ev_map[ev_name] = ev

        output = events[0].get('output', '')
        gen = events[0].get('generation', '')
        session_id = events[0].get('session_id', '')

        def get_ms(ev_dict, key='stage_elapsed_ms'):
            if not ev_dict:
                return 0.0
            val = ev_dict.get(key)
            if val is None and key == 'stage_elapsed_ms':
                val = ev_dict.get('elapsed_ms')
            return float(val) if val is not None else 0.0

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
        old_release_ms = get_ms(ev_map.get('old_pipeline_resources_released'))
        total_ms = get_ms(ev_map.get('playback_started'), key='total_elapsed_ms')
        if total_ms == 0.0:
            total_ms = get_ms(ev_map.get('playback_started'))

        results.append({
            'switch_id': switch_id,
            'output': output,
            'generation': gen,
            'session_id': session_id,
            'create_ms': create_ms,
            'set_paused_ms': set_paused_ms,
            'sink_first_frame_ms': sink_paused_ms if sink_paused_ms is not None else 0.0,
            'gst_async_done_ms': async_done_ms if async_done_ms is not None else 0.0,
            'preroll_wait_ms': preroll_wait_ms,
            'preroll_res': preroll_res,
            'set_playing_ms': set_playing_ms,
            'old_teardown_ms': old_teardown_ms,
            'old_release_ms': old_release_ms,
            'total_switch_ms': total_ms,
            'elements_paused': element_paused,
        })

    if output_csv and results:
        fieldnames = [
            'switch_id', 'output', 'generation', 'session_id',
            'create_ms', 'set_paused_ms', 'sink_first_frame_ms', 'gst_async_done_ms',
            'preroll_wait_ms', 'preroll_res', 'set_playing_ms',
            'old_teardown_ms', 'old_release_ms', 'total_switch_ms'
        ]
        with open(output_csv, 'w', newline='', encoding='utf-8') as f:
            writer = csv.DictWriter(f, fieldnames=fieldnames, extrasaction='ignore')
            writer.writeheader()
            for r in results:
                writer.writerow(r)
        print(f"Summary CSV exported to: {output_csv}")

    return results

def print_summary_table(results):
    if not results:
        print("No switch events found in log.")
        return

    print("\n=== Fluffy Integrated Video Switch Benchmark Summary ===")
    header = (
        f"{'Switch ID':<32} | {'Create':>7} | {'PrerollWait':>11} | {'Sink1stFrame':>12} | "
        f"{'AsyncDone':>9} | {'Playing':>7} | {'OldRelease':>10} | {'TotalLatency':>12}"
    )
    print(header)
    print("-" * len(header))

    for r in results:
        print(
            f"{r['switch_id']:<32} | "
            f"{r['create_ms']:>6.2f}m | "
            f"{r['preroll_wait_ms']:>10.2f}m | "
            f"{r['sink_first_frame_ms']:>11.2f}m | "
            f"{r['gst_async_done_ms']:>8.2f}m | "
            f"{r['set_playing_ms']:>6.2f}m | "
            f"{r['old_release_ms']:>9.2f}m | "
            f"{r['total_switch_ms']:>11.2f}m"
        )
    print("-" * len(header))

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <FLUFFY_LOG_PATH> [OUTPUT_CSV_PATH]")
        sys.exit(1)

    log_path = sys.argv[1]
    csv_path = sys.argv[2] if len(sys.argv) > 2 else None

    events_by_switch, _ = parse_log(log_path)
    results = analyze_switches(events_by_switch, output_csv=csv_path)
    print_summary_table(results)

if __name__ == '__main__':
    main()
