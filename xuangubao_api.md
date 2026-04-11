## 今日大盘异动

### 接口

https://api.xuangubao.com.cn/api/messages/todayDaPanYiDong?headmark=0

### 方法

GET

### 返回示例

```json
{
  "Messages": [
    {
      "Id": "1264909",
      "Title": "稳定币概念异动拉升，御银股份触及涨停，四方精创涨超8%，拉卡拉、赢时胜、翠微股份、恒宝股份、东信和平等冲高",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "56044969",
        "Name": "数字人民币"
      },
      "CreatedAt": 1775797961,
      "UpdatedAt": 1775798032
    },
    {
      "Id": "1264904",
      "Title": "算力硬件股午后拉升，永鼎股份触及涨停，源杰科技涨超10%，股价均创历史新高，东山精密、长光华芯、生益电子、新易盛纷纷冲高",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "18129294",
        "Name": "光通信"
      },
      "CreatedAt": 1775797434,
      "UpdatedAt": 1775797601
    },
    {
      "Id": "1264893",
      "Title": "商业航天概念再度活跃，陕西华达涨超15%，巨力索具、海兰信、天银机电、智明达等跟涨",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "51018737",
        "Name": "航天"
      },
      "CreatedAt": 1775791521,
      "UpdatedAt": 1775791521
    },
    {
      "Id": "1264886",
      "Title": "创新药概念异动拉升，哈三联直线涨停，三元基因、海思科、欧林生物、博瑞医药、神州细胞跟涨",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "25638417",
        "Name": "医药"
      },
      "CreatedAt": 1775789042,
      "UpdatedAt": 1775789128
    },
    {
      "Id": "1264884",
      "Title": "液冷概念股持续活跃，南方泵业拉升涨超10%，康盛股份、圣阳股份、新朋股份此前涨停，英维克、浪潮信息、同飞股份涨幅居前",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "94875777",
        "Name": "液冷服务器"
      },
      "CreatedAt": 1775788323,
      "UpdatedAt": 1775788327
    },
    {
      "Id": "1264881",
      "Title": "燃气轮机概念持续走强，联德股份3天2板，中国动力、万泽股份此前涨停，汽轮科技涨超10%，杰瑞股份、图南股份涨幅居前",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "100286110",
        "Name": "燃气轮机"
      },
      "CreatedAt": 1775787317,
      "UpdatedAt": 1775787335
    },
    {
      "Id": "1264870",
      "Title": "光伏、储能板块集体走强，德业股份涨停，固德威、艾罗能源、正泰电源、爱旭股份、阳光电源涨超5%",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "27631737",
        "Name": "储能"
      },
      "CreatedAt": 1775785415,
      "UpdatedAt": 1775785415
    },
    {
      "Id": "1264869",
      "Title": "锂电池产业链震荡拉升，国轩高科、信宇人涨停，天华新能、鹏辉能源、德赛电池、南都电源、德方纳米等跟涨",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "17032337",
        "Name": "锂电池"
      },
      "CreatedAt": 1775785176,
      "UpdatedAt": 1775785177
    },
    {
      "Id": "1264866",
      "Title": "券商股集体走强，中信证券涨超7%，广发证券、华泰证券、东方财富、国信证券、国泰海通跟涨，消息上，中信证券一季度净利润同比增长近55%",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "17006066",
        "Name": "券商"
      },
      "CreatedAt": 1775784912,
      "UpdatedAt": 1775784967
    },
    {
      "Id": "1264865",
      "Title": "存储芯片板块延续涨势，睿能科技涨停，全志科技涨超15%，盈方微、德明利、阿石创、普冉股份跟涨",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "31765737",
        "Name": "国产芯片"
      },
      "CreatedAt": 1775784749,
      "UpdatedAt": 1775784756
    },
    {
      "Id": "1264860",
      "Title": "内蒙本地股大幅高开，内蒙新华、欧晶科技涨停，蒙草生态、骑士乳业涨超10%，华能蒙电、鄂尔多斯涨超7%，消息上，国务院日前印发的《中国（内蒙古）自由贸易试验区总体方案》4月9日对外发布，赋予内蒙古自贸试验区更大改革自主权，鼓励先行先试，在更广领域、更深层次开展首创性、集成式、差别化探索",
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "22017842",
        "Name": "内蒙古概念"
      },
      "CreatedAt": 1775784600,
      "UpdatedAt": 1775784600
    }
  ],
  "NextHeadMark": "1264909"
}
```

## 板块异动

### 请求接口

https://flash-api.xuangubao.com.cn/api/event/history?count=30&types=10001,10005,10003,10007,10002,10006,10004,10008,10012,10014,10009,10010,11000,11001

### 方法

GET

### 返回示例

```json
{
  "code": 20000,
  "message": "OK",
  "data": [
    {
      "id": 8904373,
      "target": "524",
      "event_type": 11001,
      "event_timestamp": 1775804586,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 14183966,
        "plate_name": "车联网/车路云",
        "pcp": 0.009937130642741937,
        "related_stocks": [
          {
            "symbol": "688272.SS",
            "name": "富吉瑞",
            "mtm": -0.038349691021447,
            "pcp": -0.03834969102144681
          },
          {
            "symbol": "002792.SZ",
            "name": "通宇通讯",
            "mtm": -0.025420168067227,
            "pcp": -0.02068819928224608
          },
          {
            "symbol": "300857.SZ",
            "name": "协创数据",
            "mtm": -0.022208658089961003,
            "pcp": -0.09891393708623608
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904372,
      "target": "184",
      "event_type": 11001,
      "event_timestamp": 1775804586,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 20668110,
        "plate_name": "物联网",
        "pcp": 0.009015460907903778,
        "related_stocks": [
          {
            "symbol": "603042.SS",
            "name": "华脉科技",
            "mtm": -0.055269922879177,
            "pcp": -0.05526992287917731
          },
          {
            "symbol": "688450.SS",
            "name": "光格科技",
            "mtm": -0.053779807204465,
            "pcp": -0.038164002062919145
          },
          {
            "symbol": "603118.SS",
            "name": "共进股份",
            "mtm": -0.024982650936848996,
            "pcp": -0.02498265093684937
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904371,
      "target": "98",
      "event_type": 11001,
      "event_timestamp": 1775804586,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 17928146,
        "plate_name": "云计算数据中心",
        "pcp": 0.005347435647029704,
        "related_stocks": [
          {
            "symbol": "603042.SS",
            "name": "华脉科技",
            "mtm": -0.055269922879177,
            "pcp": -0.05526992287917731
          },
          {
            "symbol": "000815.SZ",
            "name": "美利云",
            "mtm": -0.041156295933366,
            "pcp": -0.025883524141363856
          },
          {
            "symbol": "002947.SZ",
            "name": "恒铭达",
            "mtm": 0.025099479644934003,
            "pcp": 0.025099479644934197
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904370,
      "target": "737",
      "event_type": 11001,
      "event_timestamp": 1775804586,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 44666049,
        "plate_name": "东数西算/算力",
        "pcp": 0.0004504074550724637,
        "related_stocks": [
          {
            "symbol": "300565.SZ",
            "name": "科信技术",
            "mtm": -0.028205128205127997,
            "pcp": -0.028205128205128216
          },
          {
            "symbol": "600589.SS",
            "name": "大位科技",
            "mtm": -0.020535714285714,
            "pcp": -0.006340579710144789
          },
          {
            "symbol": "300846.SZ",
            "name": "首都在线",
            "mtm": -0.015568068896986,
            "pcp": -0.00734802939211765
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904369,
      "target": "600568.SS",
      "event_type": 10003,
      "event_timestamp": 1775804401,
      "stock_abnormal_event_data": {
        "symbol": "600568.SS",
        "name": "ST中珠",
        "pcp": 0.04844290657439432,
        "price": 3.03,
        "related_plates": [
          {
            "plate_id": 4637726,
            "plate_name": "低价股",
            "plate_pcp": 0.005204505943262412
          },
          {
            "plate_id": 23112594,
            "plate_name": "民营医院",
            "plate_pcp": 0.0019363496142857145
          },
          {
            "plate_id": 5464089,
            "plate_name": "眼科",
            "plate_pcp": 0.006410490769230768
          },
          {
            "plate_id": 61977294,
            "plate_name": "新冠病毒防治",
            "plate_pcp": 0.007533482298198201
          },
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.01311821171686046
          },
          {
            "plate_id": 18722817,
            "plate_name": "基因测序",
            "plate_pcp": 0.01198912682
          },
          {
            "plate_id": 17236249,
            "plate_name": "房地产",
            "plate_pcp": 0.009735262502197802
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 2
    },
    {
      "id": 8904368,
      "target": "184",
      "event_type": 11001,
      "event_timestamp": 1775804400,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 20668110,
        "plate_name": "物联网",
        "pcp": 0.0091680028443299,
        "related_stocks": [
          {
            "symbol": "603042.SS",
            "name": "华脉科技",
            "mtm": -0.055269922879177,
            "pcp": -0.05526992287917731
          },
          {
            "symbol": "688450.SS",
            "name": "光格科技",
            "mtm": -0.053779807204465,
            "pcp": -0.038164002062919145
          },
          {
            "symbol": "301396.SZ",
            "name": "宏景科技",
            "mtm": -0.036590112884391,
            "pcp": -0.02606984751598629
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904367,
      "target": "384",
      "event_type": 11001,
      "event_timestamp": 1775804400,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 33504910,
        "plate_name": "腾讯概念股",
        "pcp": 0.01033942312384938,
        "related_stocks": [
          {
            "symbol": "603808.SS",
            "name": "歌力思",
            "mtm": -0.05985915492957699,
            "pcp": -0.059859154929577496
          },
          {
            "symbol": "301396.SZ",
            "name": "宏景科技",
            "mtm": -0.036590112884391,
            "pcp": -0.02606984751598629
          },
          {
            "symbol": "300857.SZ",
            "name": "协创数据",
            "mtm": -0.022208658089961003,
            "pcp": -0.09891393708623608
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904366,
      "target": "98",
      "event_type": 11001,
      "event_timestamp": 1775804400,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 17928146,
        "plate_name": "云计算数据中心",
        "pcp": 0.005612859187128714,
        "related_stocks": [
          {
            "symbol": "603042.SS",
            "name": "华脉科技",
            "mtm": -0.055269922879177,
            "pcp": -0.05526992287917731
          },
          {
            "symbol": "000815.SZ",
            "name": "美利云",
            "mtm": -0.041156295933366,
            "pcp": -0.025883524141363856
          },
          {
            "symbol": "002947.SZ",
            "name": "恒铭达",
            "mtm": 0.025099479644934003,
            "pcp": 0.025099479644934197
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904365,
      "target": "112",
      "event_type": 11000,
      "event_timestamp": 1775804400,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 18129294,
        "plate_name": "光通信",
        "pcp": -0.006328462744594591,
        "related_stocks": [
          {
            "symbol": "002962.SZ",
            "name": "五方光电",
            "mtm": 0.044471896232242,
            "pcp": 0.1001951854261549
          },
          {
            "symbol": "300504.SZ",
            "name": "天邑股份",
            "mtm": 0.030921459492888,
            "pcp": 0.03092145949288816
          },
          {
            "symbol": "300408.SZ",
            "name": "三环集团",
            "mtm": 0.02936155684534,
            "pcp": 0.029361556845339587
          }
        ]
      },
      "good_or_bad": 1
    },
    {
      "id": 8904364,
      "target": "737",
      "event_type": 11001,
      "event_timestamp": 1775804400,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 44666049,
        "plate_name": "东数西算/算力",
        "pcp": 0.0006395927036231886,
        "related_stocks": [
          {
            "symbol": "301396.SZ",
            "name": "宏景科技",
            "mtm": -0.036590112884391,
            "pcp": -0.02606984751598629
          },
          {
            "symbol": "300565.SZ",
            "name": "科信技术",
            "mtm": -0.028205128205127997,
            "pcp": -0.028205128205128216
          },
          {
            "symbol": "600589.SS",
            "name": "大位科技",
            "mtm": -0.020535714285714,
            "pcp": -0.006340579710144789
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904363,
      "target": "524",
      "event_type": 11001,
      "event_timestamp": 1775804400,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 14183966,
        "plate_name": "车联网/车路云",
        "pcp": 0.010247172449193553,
        "related_stocks": [
          {
            "symbol": "688272.SS",
            "name": "富吉瑞",
            "mtm": -0.038349691021447,
            "pcp": -0.03834969102144681
          },
          {
            "symbol": "002792.SZ",
            "name": "通宇通讯",
            "mtm": -0.025420168067227,
            "pcp": -0.02068819928224608
          },
          {
            "symbol": "300857.SZ",
            "name": "协创数据",
            "mtm": -0.022208658089961003,
            "pcp": -0.09891393708623608
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904362,
      "target": "600777.SS",
      "event_type": 10005,
      "event_timestamp": 1775804217,
      "stock_abnormal_event_data": {
        "symbol": "600777.SS",
        "name": "*ST新潮",
        "mtm": 0.0036429872495445,
        "pcp": 0.0415879017013232,
        "price": 5.51,
        "related_plates": [
          {
            "plate_id": 17204050,
            "plate_name": "石油化工",
            "plate_pcp": 0.004677919876595744
          },
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.012732681876744183
          },
          {
            "plate_id": 23406049,
            "plate_name": "油气改革",
            "plate_pcp": 0.0046061484490566035
          },
          {
            "plate_id": 18926110,
            "plate_name": "天然气",
            "plate_pcp": 0.004880070754347825
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 1
    },
    {
      "id": 8904361,
      "target": "53",
      "event_type": 11001,
      "event_timestamp": 1775804214,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 17159801,
        "plate_name": "一带一路",
        "pcp": 0.005808603804434583,
        "related_stocks": [
          {
            "symbol": "002542.SZ",
            "name": "中化岩土",
            "mtm": -0.1,
            "pcp": -0.09999999999999998
          },
          {
            "symbol": "603871.SS",
            "name": "嘉友国际",
            "mtm": -0.050167224080268,
            "pcp": 0.04488594554819714
          },
          {
            "symbol": "301182.SZ",
            "name": "凯旺科技",
            "mtm": -0.040137614678899,
            "pcp": -0.04013761467889898
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904360,
      "target": "002787.SZ",
      "event_type": 10007,
      "event_timestamp": 1775804214,
      "stock_abnormal_event_data": {
        "symbol": "002787.SZ",
        "name": "华源控股",
        "pcp": 0.0997605746209098,
        "price": 13.78,
        "related_plates": [
          {
            "plate_id": 7198750,
            "plate_name": "回购",
            "plate_pcp": 0.007452874711180123
          },
          {
            "plate_id": 18811806,
            "plate_name": "包装印刷",
            "plate_pcp": 0.010414855343333336
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 2
    },
    {
      "id": 8904359,
      "target": "300131.SZ",
      "event_type": 10001,
      "event_timestamp": 1775804214,
      "stock_abnormal_event_data": {
        "symbol": "300131.SZ",
        "name": "英唐智控",
        "pcp": 0.19999999999999996,
        "price": 15.42,
        "related_plates": [
          {
            "plate_id": 66814321,
            "plate_name": "碳化硅",
            "plate_pcp": 0.019104370995454544
          },
          {
            "plate_id": 41380009,
            "plate_name": "激光雷达",
            "plate_pcp": 0.011776870170833335
          },
          {
            "plate_id": 1008158,
            "plate_name": "被动元件",
            "plate_pcp": 0.009224800481250003
          },
          {
            "plate_id": 31765737,
            "plate_name": "国产芯片",
            "plate_pcp": 0.0113552205628
          },
          {
            "plate_id": 54594386,
            "plate_name": "无线耳机",
            "plate_pcp": 0.01791953307956989
          },
          {
            "plate_id": 30370802,
            "plate_name": "小米概念股",
            "plate_pcp": 0.013320959112676056
          },
          {
            "plate_id": 20668110,
            "plate_name": "物联网",
            "plate_pcp": 0.008795176266323026
          },
          {
            "plate_id": 33246089,
            "plate_name": "阿里巴巴概念股",
            "plate_pcp": 0.00925425467272727
          },
          {
            "plate_id": 39480658,
            "plate_name": "汽车芯片",
            "plate_pcp": 0.014915456083582088
          },
          {
            "plate_id": 33504910,
            "plate_name": "腾讯概念股",
            "plate_pcp": 0.010182104172803352
          },
          {
            "plate_id": 88981697,
            "plate_name": "智能座舱",
            "plate_pcp": 0.014896490315942026
          },
          {
            "plate_id": 37087250,
            "plate_name": "第三代半导体",
            "plate_pcp": 0.017393382527
          },
          {
            "plate_id": 18580382,
            "plate_name": "OLED",
            "plate_pcp": 0.008401093407446806
          },
          {
            "plate_id": 16988830,
            "plate_name": "无人驾驶",
            "plate_pcp": 0.01132555432172285
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 1
    },
    {
      "id": 8904358,
      "target": "600777.SS",
      "event_type": 10005,
      "event_timestamp": 1775804208,
      "stock_abnormal_event_data": {
        "symbol": "600777.SS",
        "name": "*ST新潮",
        "mtm": 0.0036429872495445,
        "pcp": 0.0415879017013232,
        "price": 5.51,
        "related_plates": [
          {
            "plate_id": 23406049,
            "plate_name": "油气改革",
            "plate_pcp": 0.0046061484490566035
          },
          {
            "plate_id": 18926110,
            "plate_name": "天然气",
            "plate_pcp": 0.004880070754347825
          },
          {
            "plate_id": 17204050,
            "plate_name": "石油化工",
            "plate_pcp": 0.004677919876595744
          },
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.012732681876744183
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 1
    },
    {
      "id": 8904357,
      "target": "002808.SZ",
      "event_type": 10005,
      "event_timestamp": 1775804205,
      "stock_abnormal_event_data": {
        "symbol": "002808.SZ",
        "name": "*ST恒久",
        "mtm": 0.0064935064935065,
        "pcp": 0.04494382022471921,
        "price": 4.65,
        "related_plates": [
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.012732681876744183
          },
          {
            "plate_id": 40572622,
            "plate_name": "NFT",
            "plate_pcp": 0.010537133330909092
          },
          {
            "plate_id": 25513273,
            "plate_name": "军工",
            "plate_pcp": 0.007300496833417721
          },
          {
            "plate_id": 18693774,
            "plate_name": "军民融合",
            "plate_pcp": 0.006405771642105262
          },
          {
            "plate_id": 97015161,
            "plate_name": "区块链",
            "plate_pcp": 0.013911686570500004
          },
          {
            "plate_id": 43341297,
            "plate_name": "元宇宙",
            "plate_pcp": 0.00815287776608696
          },
          {
            "plate_id": 17752718,
            "plate_name": "网络安全",
            "plate_pcp": 0.009762910467567562
          },
          {
            "plate_id": 38844242,
            "plate_name": "信创",
            "plate_pcp": 0.010637745797512432
          },
          {
            "plate_id": 1277665,
            "plate_name": "应急产业",
            "plate_pcp": 0.007257278955882355
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 1
    },
    {
      "id": 8904356,
      "target": "000929.SZ",
      "event_type": 10003,
      "event_timestamp": 1775804205,
      "stock_abnormal_event_data": {
        "symbol": "000929.SZ",
        "name": "*ST兰黄",
        "pcp": 0.04988913525498906,
        "price": 9.47,
        "related_plates": [
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.012732681876744183
          },
          {
            "plate_id": 19939217,
            "plate_name": "啤酒",
            "plate_pcp": 0.00735437661
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 2
    },
    {
      "id": 8904355,
      "target": "000929.SZ",
      "event_type": 10001,
      "event_timestamp": 1775804202,
      "stock_abnormal_event_data": {
        "symbol": "000929.SZ",
        "name": "*ST兰黄",
        "pcp": 0.04988913525498906,
        "price": 9.47,
        "related_plates": [
          {
            "plate_id": 19939217,
            "plate_name": "啤酒",
            "plate_pcp": 0.00735437661
          },
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.012732681876744183
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 1
    },
    {
      "id": 8904354,
      "target": "300131.SZ",
      "event_type": 10005,
      "event_timestamp": 1775804196,
      "stock_abnormal_event_data": {
        "symbol": "300131.SZ",
        "name": "英唐智控",
        "mtm": -0.003242542153048,
        "pcp": 0.1961089494163424,
        "price": 15.37,
        "related_plates": [
          {
            "plate_id": 66814321,
            "plate_name": "碳化硅",
            "plate_pcp": 0.019104370995454544
          },
          {
            "plate_id": 37087250,
            "plate_name": "第三代半导体",
            "plate_pcp": 0.017393382527
          },
          {
            "plate_id": 30370802,
            "plate_name": "小米概念股",
            "plate_pcp": 0.013320959112676056
          },
          {
            "plate_id": 31765737,
            "plate_name": "国产芯片",
            "plate_pcp": 0.0113552205628
          },
          {
            "plate_id": 16988830,
            "plate_name": "无人驾驶",
            "plate_pcp": 0.01132555432172285
          },
          {
            "plate_id": 88981697,
            "plate_name": "智能座舱",
            "plate_pcp": 0.014896490315942026
          },
          {
            "plate_id": 39480658,
            "plate_name": "汽车芯片",
            "plate_pcp": 0.014915456083582088
          },
          {
            "plate_id": 20668110,
            "plate_name": "物联网",
            "plate_pcp": 0.008795176266323026
          },
          {
            "plate_id": 33504910,
            "plate_name": "腾讯概念股",
            "plate_pcp": 0.010182104172803352
          },
          {
            "plate_id": 54594386,
            "plate_name": "无线耳机",
            "plate_pcp": 0.01791953307956989
          },
          {
            "plate_id": 18580382,
            "plate_name": "OLED",
            "plate_pcp": 0.008401093407446806
          },
          {
            "plate_id": 41380009,
            "plate_name": "激光雷达",
            "plate_pcp": 0.011776870170833335
          },
          {
            "plate_id": 33246089,
            "plate_name": "阿里巴巴概念股",
            "plate_pcp": 0.00925425467272727
          },
          {
            "plate_id": 1008158,
            "plate_name": "被动元件",
            "plate_pcp": 0.009224800481250003
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 1
    },
    {
      "id": 8904352,
      "target": "000929.SZ",
      "event_type": 10003,
      "event_timestamp": 1775804193,
      "stock_abnormal_event_data": {
        "symbol": "000929.SZ",
        "name": "*ST兰黄",
        "mtm": -0.0010559662090813,
        "pcp": 0.04878048780487809,
        "price": 9.46,
        "related_plates": [
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.012732681876744183
          },
          {
            "plate_id": 19939217,
            "plate_name": "啤酒",
            "plate_pcp": 0.00735437661
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 2
    },
    {
      "id": 8904351,
      "target": "600777.SS",
      "event_type": 10005,
      "event_timestamp": 1775804191,
      "stock_abnormal_event_data": {
        "symbol": "600777.SS",
        "name": "*ST新潮",
        "mtm": 0.0036429872495445,
        "pcp": 0.0415879017013232,
        "price": 5.51,
        "related_plates": [
          {
            "plate_id": 23406049,
            "plate_name": "油气改革",
            "plate_pcp": 0.0046061484490566035
          },
          {
            "plate_id": 18926110,
            "plate_name": "天然气",
            "plate_pcp": 0.004880070754347825
          },
          {
            "plate_id": 17204050,
            "plate_name": "石油化工",
            "plate_pcp": 0.004677919876595744
          },
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.012732681876744183
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 1
    },
    {
      "id": 8904353,
      "target": "603580.SS",
      "event_type": 10007,
      "event_timestamp": 1775804190,
      "stock_abnormal_event_data": {
        "symbol": "603580.SS",
        "name": "*ST艾艾",
        "pcp": 0.05008944543828253,
        "price": 17.61,
        "related_plates": [
          {
            "plate_id": 17957649,
            "plate_name": "工业自动化",
            "plate_pcp": 0.01205386367425743
          },
          {
            "plate_id": 29751954,
            "plate_name": "智能制造",
            "plate_pcp": 0.0093362604093361
          },
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.012732681876744183
          },
          {
            "plate_id": 17428174,
            "plate_name": "强势人气股",
            "plate_pcp": -0.011767743915116284
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 2
    },
    {
      "id": 8904350,
      "target": "281",
      "event_type": 11001,
      "event_timestamp": 1775804175,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 25764977,
        "plate_name": "新能源汽车",
        "pcp": 0.017305727322391315,
        "related_stocks": [
          {
            "symbol": "688667.SS",
            "name": "菱电电控",
            "mtm": -0.053773584905660005,
            "pcp": -0.05377358490566032
          },
          {
            "symbol": "300565.SZ",
            "name": "科信技术",
            "mtm": -0.028205128205127997,
            "pcp": -0.028205128205128216
          },
          {
            "symbol": "300988.SZ",
            "name": "津荣天宇",
            "mtm": -0.022396018485603,
            "pcp": 0.0043827611395179655
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904349,
      "target": "567",
      "event_type": 11001,
      "event_timestamp": 1775804175,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 53171153,
        "plate_name": "国企改革",
        "pcp": 0.005779559248060346,
        "related_stocks": [
          {
            "symbol": "002207.SZ",
            "name": "准油股份",
            "mtm": -0.04505632040050101,
            "pcp": -0.045056320400500664
          },
          {
            "symbol": "000815.SZ",
            "name": "美利云",
            "mtm": -0.041156295933366,
            "pcp": -0.025883524141363856
          },
          {
            "symbol": "300393.SZ",
            "name": "中来股份",
            "mtm": 0.016815034619188998,
            "pcp": 0.007843137254901933
          }
        ]
      },
      "good_or_bad": 2
    },
    {
      "id": 8904348,
      "target": "603459.SS",
      "event_type": 10003,
      "event_timestamp": 1775804161,
      "stock_abnormal_event_data": {
        "symbol": "603459.SS",
        "name": "C红板",
        "mtm": -0.010556450169977,
        "pcp": 0.10005967774020272,
        "price": 55.3
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 2
    },
    {
      "id": 8904347,
      "target": "600488.SS",
      "event_type": 10006,
      "event_timestamp": 1775804154,
      "stock_abnormal_event_data": {
        "symbol": "600488.SS",
        "name": "津药药业",
        "mtm": -0.013824884792627,
        "pcp": -0.09322033898305083,
        "price": 6.42,
        "related_plates": [
          {
            "plate_id": 17428174,
            "plate_name": "强势人气股",
            "plate_pcp": -0.011144336830232558
          },
          {
            "plate_id": 25638417,
            "plate_name": "医药",
            "plate_pcp": 0.004786382529618768
          },
          {
            "plate_id": 26676721,
            "plate_name": "化学原料药",
            "plate_pcp": 0.005414938846511629
          },
          {
            "plate_id": 61977294,
            "plate_name": "新冠病毒防治",
            "plate_pcp": 0.007413392904954951
          },
          {
            "plate_id": 35716402,
            "plate_name": "辅助生殖",
            "plate_pcp": 0.00303221264516129
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 2
    },
    {
      "id": 8904346,
      "target": "301396.SZ",
      "event_type": 10010,
      "event_timestamp": 1775804142,
      "stock_abnormal_event_data": {
        "symbol": "301396.SZ",
        "name": "宏景科技",
        "mtm": -0.04023184452778701,
        "pcp": -0.030742744712247916,
        "price": 197.05,
        "related_plates": [
          {
            "plate_id": 13011026,
            "plate_name": "高价股",
            "plate_pcp": 0.016335953380172418
          },
          {
            "plate_id": 38844242,
            "plate_name": "信创",
            "plate_pcp": 0.010798807332835822
          },
          {
            "plate_id": 59173721,
            "plate_name": "智慧政务",
            "plate_pcp": 0.006916432072413792
          },
          {
            "plate_id": 29751954,
            "plate_name": "智能制造",
            "plate_pcp": 0.009434921006638992
          },
          {
            "plate_id": 84058014,
            "plate_name": "ChatGPT",
            "plate_pcp": 0.00895340754090909
          },
          {
            "plate_id": 17548129,
            "plate_name": "VR\u0026AR",
            "plate_pcp": 0.011783501434394904
          },
          {
            "plate_id": 27711182,
            "plate_name": "智慧物流",
            "plate_pcp": 0.00995127588219178
          },
          {
            "plate_id": 20668110,
            "plate_name": "物联网",
            "plate_pcp": 0.00903270847491409
          },
          {
            "plate_id": 21553249,
            "plate_name": "智慧城市",
            "plate_pcp": 0.006264769329153608
          },
          {
            "plate_id": 20922194,
            "plate_name": "大数据",
            "plate_pcp": 0.01003009045201794
          },
          {
            "plate_id": 44666049,
            "plate_name": "东数西算/算力",
            "plate_pcp": 0.0011018106304347818
          },
          {
            "plate_id": 33504910,
            "plate_name": "腾讯概念股",
            "plate_pcp": 0.010277700561924688
          },
          {
            "plate_id": 16853682,
            "plate_name": "国产软件",
            "plate_pcp": 0.009850572974166673
          },
          {
            "plate_id": 17490281,
            "plate_name": "人工智能",
            "plate_pcp": 0.009475199090220826
          },
          {
            "plate_id": 30605897,
            "plate_name": "数字经济",
            "plate_pcp": 0.00935055834921136
          },
          {
            "plate_id": 5967502,
            "plate_name": "医疗信息化",
            "plate_pcp": 0.006348700107894736
          },
          {
            "plate_id": 45837518,
            "plate_name": "新型城镇化",
            "plate_pcp": 0.007717450551219514
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 2
    },
    {
      "id": 8904345,
      "target": "600777.SS",
      "event_type": 10005,
      "event_timestamp": 1775804142,
      "stock_abnormal_event_data": {
        "symbol": "600777.SS",
        "name": "*ST新潮",
        "mtm": 0.0018181818181818,
        "pcp": 0.0415879017013232,
        "price": 5.51,
        "related_plates": [
          {
            "plate_id": 24898553,
            "plate_name": "ST股",
            "plate_pcp": 0.012780901099418604
          },
          {
            "plate_id": 23406049,
            "plate_name": "油气改革",
            "plate_pcp": 0.004803966711320755
          },
          {
            "plate_id": 18926110,
            "plate_name": "天然气",
            "plate_pcp": 0.004883999417391303
          },
          {
            "plate_id": 17204050,
            "plate_name": "石油化工",
            "plate_pcp": 0.004655621046808511
          }
        ]
      },
      "plate_abnormal_event_data": {},
      "good_or_bad": 1
    },
    {
      "id": 8904344,
      "target": "79",
      "event_type": 11001,
      "event_timestamp": 1775804139,
      "stock_abnormal_event_data": {},
      "plate_abnormal_event_data": {
        "plate_id": 17548129,
        "plate_name": "VR\u0026AR",
        "pcp": 0.011783501434394904,
        "related_stocks": [
          {
            "symbol": "300331.SZ",
            "name": "苏大维格",
            "mtm": -0.049555555555555,
            "pcp": -0.04955555555555547
          },
          {
            "symbol": "301396.SZ",
            "name": "宏景科技",
            "mtm": -0.03701719351225,
            "pcp": -0.027496310870634533
          },
          {
            "symbol": "300812.SZ",
            "name": "易天股份",
            "mtm": -0.02188679245283,
            "pcp": -0.021886792452830095
          }
        ]
      },
      "good_or_bad": 2
    }
  ]
}
```
