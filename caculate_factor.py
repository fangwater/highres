import numpy as np 
import pandas as pd 
import os 
import matplotlib.pyplot as plt 

pd.options.mode.chained_assignment = None 

# 配置路径 
outputpath = '/home/u171/mth_pub/matched_orders' 
figpath = '/home/u171/mth_pub/figures' 
os.makedirs(figpath, exist_ok=True)

# 币种列表
symbols = sorted([
    "AAVEUSDT", "ADAUSDT", "ARBUSDT", "ATOMUSDT", "AVAXUSDT", 
    "BCHUSDT", "BNBUSDT", "BTCUSDT", "DOGEUSDT", "DOTUSDT", 
    "ETCUSDT", "ETHUSDT", "FILUSDT", "HBARUSDT", "LINKUSDT", 
    "LTCUSDT", "SOLUSDT", "TONUSDT", "TRXUSDT", "WLDUSDT", 
    "XLMUSDT", "XRPUSDT"
])

mstr = "1m"

# 读取所有数据
for symbol in symbols:
    input_file = os.path.join(outputpath, f"{symbol}_{mstr}.h5")
    
    if not os.path.exists(input_file): continue
    
    print(f"读取 {symbol}...")
    nms = pd.read_hdf(input_file, key="df")
    
    if len(nms) == 0:
        print(f"警告: {symbol} 数据为空,跳过")
        continue

    order_df = nms.rename(columns={'cts': 'ts'}).set_index('ts')[
        ['pnlu', 'pnlu_wfee', 'tlen', 'holding', 'holding_close', 'range', 'crange', 'side']
    ]

    factor_df2 = (order_df.groupby('ts')[['pnlu']].count() * order_df.groupby('ts')[['pnlu']].mean())
    ts_df = pd.DataFrame({'ts': list(range(factor_df2.index.min(), factor_df2.index.max()+1, 5))}).set_index('ts')
    factor_df2 = pd.merge(ts_df, factor_df2, left_index=True, right_index=True, how='left')
    factor_df2['ylabel'] = factor_df2['pnlu'].rolling(720, min_periods=300).mean().shift(120)
    factor_df2 = factor_df2[['ylabel']].ffill()