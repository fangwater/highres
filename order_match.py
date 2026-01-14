# ==================== 文件1: 数据处理和存储 ====================
import numpy as np 
import pandas as pd 
import os 

pd.options.mode.chained_assignment = None 

# 配置路径 
root_path = '/home/u171/mth_pub/order_data' 
outputpath = '/home/u171/mth_pub/matched_orders' 
os.makedirs(outputpath, exist_ok=True) 

# 币种列表
symbols = sorted([
    "AAVEUSDT", "ADAUSDT", "ARBUSDT", "ATOMUSDT", "AVAXUSDT", 
    "BCHUSDT", "BNBUSDT", "BTCUSDT", "DOGEUSDT", "DOTUSDT", 
    "ETCUSDT", "ETHUSDT", "FILUSDT", "HBARUSDT", "LINKUSDT", 
    "LTCUSDT", "SOLUSDT", "TONUSDT", "TRXUSDT", "WLDUSDT", 
    "XLMUSDT", "XRPUSDT"
])

mstr = "1m"
fee = 0.0001

for symbol in symbols:
    output_file = os.path.join(outputpath, f"{symbol}_{mstr}.h5")
    
    order_file_path = os.path.join(root_path, f"{symbol}_orders.csv")
    
    if not os.path.exists(order_file_path):
        print(f"文件不存在: {order_file_path},跳过")
        continue
    
    print(f"处理 {symbol}, 读取文件: {order_file_path}")
    
    try:
        ns = pd.read_csv(order_file_path, header=None) 
        ns.columns = ["oid", "cts", "uts", "oid2", "symbol", "ttype", "sid", "side", "price", "amount", "famount", "status", "inpos", "tlen", "fkey", "bid1", "ask1"]
        ns.sort_values('uts', inplace=True)
        
    except (pd.errors.EmptyDataError, FileNotFoundError) as e:
        print(f"警告: 文件 {order_file_path} 读取失败: {e}, 跳过")
        continue
    
    if len(ns) == 0:
        print(f"警告: {symbol} 数据为空,跳过")
        continue
    
    # 时间戳转换 
    ns["cts"] = (ns["cts"] / 1000).astype("int") 
    ns["uts"] = (ns["uts"] / 1000).astype("int") 
    
    # 开仓订单处理
    ns_open = ns[(ns["oid"].str.startswith("o")) & (ns["sid"] == 0)] 
    
    if len(ns_open) == 0:
        print(f"警告: {symbol} 没有开仓订单,跳过") 
        continue 
    
    ns_open["holding"] = ns_open['uts'] - ns_open['cts'] 
    print(f'开仓持有时间均值: {ns_open["holding"].mean():.2f}秒') 
    
    ns_open["range"] = (ns_open["oid"].str.split("_", expand=True)[2].astype("float") * 10000).round(0)
    ranges = ns_open["range"].drop_duplicates().tolist() 
    print(f'Range列表: {ranges}') 
    
    prices = ns_open.groupby("oid").last().reset_index()[
        ["cts", "uts", 'holding', "oid", "price", "fkey", "side", "range", 'tlen']
    ]
    prices["open_uts"] = prices["uts"]
    prices["tlen"] = prices["tlen"] * prices["price"] 
    
    famount = ns_open.groupby("oid")["famount"].sum().reset_index() 
    ns_open_m = pd.merge(prices, famount, on=["oid"], how="left") 
    
    # 平仓订单处理
    ns_close = ns[(ns["oid"].str.startswith("c")) & (ns["sid"] == 1)].sort_values('uts')
    
    if len(ns_close) == 0:
        print(f"警告: {symbol} 没有平仓订单,跳过")
        continue
    
    ns_close["fkey"] = ns_close["fkey"].str.split("_", expand=True)[2]
    ns_close["crange"] = (ns_close["oid"].str.split("_", expand=True)[5].astype("float") * 10000).round(0)
    
    fts = ns_close.groupby("fkey")[["uts", "crange"]].last().reset_index()
    count = ns_close.groupby("fkey")[["uts"]].count().reset_index()
    count.columns = ["fkey", "close_count"]
    fts = pd.merge(fts, count, left_on='fkey', right_on='fkey', how='inner')
    fts["uts"] = fts["uts"].astype("int") 
    fts.columns = ["fkey", "fts", "crange", "close_count"] 
    
    famounts = ns_close.groupby("fkey")["famount"].sum().reset_index()
    famounts.columns = ["fkey", "camount"]
    
    ns_close["pa"] = ns_close["famount"] * ns_close["price"]
    pas = ns_close.groupby("fkey")["pa"].sum().reset_index()
    pas = pd.merge(pas, fts, on=["fkey"], how="left")
    nm = pd.merge(pas, famounts, on=["fkey"], how="left")
    nm["cprice"] = nm["pa"] / nm["camount"]
    
    # 合并开仓和平仓数据
    nms = pd.merge(ns_open_m, nm, on="fkey", how="left").dropna()
    
    if len(nms) == 0:
        print(f"警告: {symbol} 匹配后数据为空,跳过")
        continue
    
    # 计算PnL
    nms.loc[nms["side"] == "buy", "pnlu"] = (
        (nms[nms["side"] == "buy"]["cprice"] - nms[nms["side"] == "buy"]["price"]) / 
        nms[nms["side"] == "buy"]["price"]
    )
    nms.loc[nms["side"] == "sell", "pnlu"] = (
        (nms[nms["side"] == "sell"]["price"] - nms[nms["side"] == "sell"]["cprice"]) / 
        nms[nms["side"] == "sell"]["price"]
    )
    nms["pnlu_wfee"] = nms["pnlu"] - 2 * fee
    
    nms["fts"] = nms["fts"].astype("int")
    nms["holding_close"] = nms['fts'] - nms['open_uts']
    
    print(f'平仓持有时间均值: {nms["holding_close"].mean():.2f}秒')
    print(f'平均PnL: {nms["pnlu"].mean():.6f}')
    
    # 整理输出列
    nms = nms[[
        'cts', 'open_uts', 'fts', 'holding', 'holding_close', 'close_count',
        'price', 'cprice', 'camount', 'side', 'range', 'crange', 'tlen',
        'pnlu', 'pnlu_wfee'
    ]] 
    nms.sort_values('cts', inplace=True) 
    
    print(f"\n总记录数: {len(nms)}, 总体平均PnL: {nms.pnlu.mean():.6f}")
    
    # 保存结果
    nms.to_hdf(output_file, key="df", mode='w')
    print(f"数据已保存: {output_file}\n")
    print("=" * 80)

print("\n所有数据处理完成!")
