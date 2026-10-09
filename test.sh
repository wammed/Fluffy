```bash
cd /home/susie/git/wammed/Fluffy

D=target/benchmarks/20261010_010316_21278

echo '===== 1. 生成されたファイル ====='
find "$D" -maxdepth 3 -type f -printf '%TY-%Tm-%Td %TH:%TM:%TS %s %p\n' | sort

echo '===== 2. 解析CSV ====='
if test -f "$D/switch_summary.csv"; then
  cat "$D/switch_summary.csv"
else
  echo 'switch_summary.csv がありません'
fi

echo '===== 3. ログ中の切り替えイベント ====='
grep -nE 'video_switch_committed|playback_started|preroll_wait_returned|pipeline_set_playing_returned|old_pipeline_teardown_completed' \
  "$D/fluffy_daemon.log" | tail -n 50

echo '===== 4. 解析器のイベント判定 ====='
grep -nE 'No switch events found|switch_summary|playback_started|video_switch_committed|switch event' \
  scripts/analyze_switch_bench.py

echo '===== 5. 計測終了処理 ====='
grep -nE 'Stopping telemetry|wait|kill|trap|END_EPOCH|collector|analyze_switch' \
  scripts/run_integrated_bench.sh scripts/collect_telemetry.sh
```
