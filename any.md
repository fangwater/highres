以eth为例子
  - 开仓总数 open_total=132513
  - 能匹配到平仓的开仓 open_matched=132492
  - 未匹配开仓 open_unmatched=21
  - 平仓 fkey 总数 close_total=132492
  - 未匹配平仓 close_unmatched=0
  - 平仓时间 - 开仓时间最大差值 8658 秒（约 2小时24分18秒）
  • 已分析定位：21 条未匹配开仓的索引非常集中在文件末尾。

  - 最小行索引：832087
  - 最大行索引：832274
  - 文件总行数：832289
  - 21 条全部落在最后 1% 行内（unmatched_in_last_1pct=21）
大部分订单是可以匹配成功的，基本不需要drop


基本逻辑，根据fkey，现有开仓才有平仓
1、perid interval 我多久进行一次报单，设置为5s
2、建立一个哈希表，维护一个range。[min_t , max_t] <= window_period 设置为30min，满足目前的rolling window
用目前的order match的规则，分两个map 1800/5 = 360
3、一个阈值，shift_period，默认120，即10分钟

按照
/home/fanghaizhou/ex_check/highres/order_match.py的逻辑，/home/fanghaizhou/ex_check/highres/caculate_factor.py
构造流式处理的版本

1、逐条输入订单，从csv中读取
2、每条订单的时间，可以对齐到秒，获得一个current tp
3、tp跨越period，说明到了下一个period的报单
4、从哈希表中，获取period = 当前period - shift的开仓单，以及对应的平仓单（找不到drop）
按 ts 聚合 pnlu：count * mean， 聚合得到


已用流式方式实现：逐条读 CSV、5 秒一桶、30 分钟滚动窗口、shift=120（10 分钟），按 fkey 只在有开仓
  时匹配平仓，找不到就 drop，同时输出与 caculate_factor.py 一致的滚动/shift 标签。

  - 新脚本：stream_order_factor.py
  - 逻辑：cts 用于开仓分桶，uts 推进 period；每个 period 计算 target_ts = ts - shift 的开仓匹配平
    仓，pnlu_sum = count * mean（等价于 sum）。
  - 输出 CSV：ts, target_ts, pnlu_sum, pnlu_count, ylabel（滚动均值+ffill）

  可行性：count * mean 等于 sum，作为聚合是可行的；缺失桶会被当作 NaN，不进入滚动均值，避免拉低结
  果。

  下一步建议：

  1. 运行：python stream_order_factor.py --input ETHUSDT_orders.csv
  2. 如需不同窗口：调整 --window、--shift、--rolling-window、--min-periods 参数。







 


