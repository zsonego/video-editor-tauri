# AIcut 客户端接口文档

**HOST**：`http://localhost:8080`

**接口分组**：`Aicut客户端接口`

**接口路径**：`/v3/api-docs?group=Aicut客户端接口`

## 1. 通用说明

### 1.1 普通接口返回

```json
{
  "code": 0,
  "msg": "操作成功",
  "data": {}
}
```

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| code | Integer | 接口状态码。`0` 成功，`1` 业务失败，`500` 等为系统异常 |
| msg | String | 结果描述 |
| data | Object/Array | 返回数据，具体结构见各接口 |

### 1.2 分页列表返回

分页列表统一按若依格式返回：

```json
{
  "code": 0,
  "msg": "查询成功",
  "rows": [],
  "total": 0
}
```

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| code | Integer | 接口状态码 |
| msg | String | 结果描述 |
| rows | Array | 当前页数据 |
| total | Long | 总记录数 |

当前使用分页列表格式的客户端接口：

- `POST /api/project/my`
- `POST /api/template/list`

### 1.3 租户字段

接口请求体里租户字段优先使用：

```json
{
  "renter_id": "default"
}
```

后端兼容 `renterId`。

## 2. AI剪辑工程接口

### 2.1 新建工程基本信息

**接口地址**：`/api/project/create`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：输入工程名称、模板 id、租户 id、用户 id，创建编辑中的剪辑工程。

**请求示例**：

```json
{
  "projectName": "顾小童周岁纪念",
  "renter_id": "default",
  "templateId": 1,
  "userId": 1
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| projectName | 是 | String | 工程名称 |
| renter_id | 是 | String | 租户 id，兼容 `renterId` |
| templateId | 是 | Long | 模板 id |
| userId | 是 | Long | 用户 id |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data.projectId | Long | 新建工程 id |

**响应示例**：

```json
{
  "code": 0,
  "msg": "创建成功",
  "data": {
    "projectId": 1
  }
}
```

### 2.2 获取工程信息

**接口地址**：`/api/project/detail`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：输入工程 id，返回工程基本信息、导出扣减积分值、模板 id 和工程文件地址。

**请求示例**：

```json
{
  "projectId": 1
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| projectId | 是 | Long | 工程 id |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data.projectId | Long | 工程 id |
| data.projectName | String | 工程名称 |
| data.status | Integer | 工程状态 |
| data.statusName | String | 工程状态名称，如 `编辑中`、`已导出` |
| data.exportCredit | Integer | 导出扣减积分值 |
| data.templateId | Long | 模板 id |
| data.projectFileUrl | String | 工程文件地址 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "查询成功",
  "data": {
    "projectId": 1,
    "projectName": "顾小童周岁纪念",
    "status": 0,
    "statusName": "编辑中",
    "exportCredit": 10,
    "templateId": 1,
    "projectFileUrl": "project/1.xml"
  }
}
```

### 2.3 查询我的工程

**接口地址**：`/api/project/my`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：输入租户 id、用户 id，分页返回当前用户的剪辑工程列表。

**请求示例**：

```json
{
  "renter_id": "default",
  "userId": 1,
  "pageNum": 1,
  "pageSize": 10
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| renter_id | 是 | String | 租户 id，兼容 `renterId` |
| userId | 是 | Long | 用户 id |
| pageNum | 否 | Integer | 页码，从 1 开始，默认 1 |
| pageSize | 否 | Integer | 每页条数，默认 10，最大 100 |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| rows | Array | 当前页工程列表 |
| rows[].projectId | Long | 工程 id |
| rows[].projectName | String | 工程名称 |
| rows[].status | Integer | 工程状态 |
| rows[].statusName | String | 工程状态名称 |
| total | Long | 总记录数 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "查询成功",
  "rows": [
    {
      "projectId": 1,
      "projectName": "顾小童周岁纪念",
      "status": 0,
      "statusName": "编辑中"
    }
  ],
  "total": 1
}
```

### 2.4 更新工程信息

**接口地址**：`/api/project/update`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：JSON 方式更新工程名称、模板 id 和工程 XML 内容。

**请求示例**：

```json
{
  "projectId": 1,
  "projectName": "顾小童周岁纪念",
  "projectXml": "<project id=\"demo\" name=\"顾小童周岁纪念\"></project>",
  "templateId": 1
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| projectId | 是 | Long | 工程 id |
| projectName | 是 | String | 工程名称 |
| projectXml | 是 | String | 工程 XML 内容 |
| templateId | 是 | Long | 模板 id |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data | Object | 空对象 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "更新成功",
  "data": {}
}
```

## 3. AI剪辑模板接口

### 3.1 查询剪辑模板类别

**接口地址**：`/api/template/category`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：输入租户 id 备用，返回模板类别 id 和类别名称。

**请求示例**：

```json
{
  "renter_id": "default"
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| renter_id | 否 | String | 租户 id，备用，兼容 `renterId` |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data | Array | 模板类别列表 |
| data[].categoryId | Integer | 模板类别 id |
| data[].categoryName | String | 类别名称 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "查询成功",
  "data": [
    {
      "categoryId": 1,
      "categoryName": "新生儿上门"
    },
    {
      "categoryId": 2,
      "categoryName": "店内微电影"
    }
  ]
}
```

### 3.2 查询剪辑模板

**接口地址**：`/api/template/list`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：按类别、关键字、收藏状态、排序方式查询启用的剪辑模板。

**请求示例**：

```json
{
  "favorite": 1,
  "keyword": "旅拍",
  "pageNum": 1,
  "pageSize": 10,
  "renter_id": "default",
  "sortType": 0,
  "templateCategoryId": 1,
  "userId": 1
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| renter_id | 是 | String | 租户 id，兼容 `renterId` |
| userId | 是 | Long | 用户 id |
| templateCategoryId | 否 | Integer | 模板类别 id，兼容 `categoryId` |
| keyword | 否 | String | 模板关键字，匹配模板名称或标签 |
| favorite | 否 | Integer | 是否收藏。`1` 只查已收藏，`0` 只查未收藏，不传查全部 |
| sortType | 否 | Integer | 排序方式：`0` 模板名称升序，`1` 推荐值降序，`2` 使用量降序 |
| pageNum | 否 | Integer | 页码，从 1 开始，默认 1 |
| pageSize | 否 | Integer | 每页条数，默认 10，最大 100 |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| rows | Array | 当前页模板列表 |
| rows[].templateId | Long | 模板 id |
| rows[].templateName | String | 模板名称 |
| rows[].coverPic | String | 模板缩略图地址 |
| rows[].assetsCount | Integer | 包含素材集数 |
| rows[].mediaCount | Integer | 需要视频素材总数 |
| rows[].videoDuration | Integer | 模板总时长，单位秒 |
| rows[].favorite | Integer | 当前用户是否收藏：`0` 未收藏，`1` 已收藏 |
| rows[].usageCount | Long | 模板使用量 |
| total | Long | 总记录数 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "查询成功",
  "rows": [
    {
      "templateId": 1,
      "templateName": "新生儿百日纪念快剪模板",
      "coverPic": "common/newborn-cover.png",
      "assetsCount": 3,
      "mediaCount": 8,
      "videoDuration": 60,
      "favorite": 1,
      "usageCount": 6
    }
  ],
  "total": 1
}
```

### 3.3 获取剪辑模板详情

**接口地址**：`/api/template/detail`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：输入模板 id，返回模板版本号、XML 文件地址和素材压缩包地址。

**请求示例**：

```json
{
  "templateId": 1
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| templateId | 是 | Long | 模板 id |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data.templateId | Long | 模板 id |
| data.version | String | 模板版本号 |
| data.xmlPath | String | 模板 XML 文件地址 |
| data.assetsPath | String | 模板素材压缩包地址 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "查询成功",
  "data": {
    "templateId": 1,
    "version": "1.0",
    "xmlPath": "common/template.xml",
    "assetsPath": "common/template-assets.zip"
  }
}
```

### 3.4 收藏/取消收藏剪辑模板

**接口地址**：`/api/template/favorite`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：输入模板 id、租户 id、用户 id 和动作，`0` 收藏，`1` 取消收藏。

**请求示例**：

```json
{
  "action": 0,
  "renter_id": "default",
  "templateId": 1,
  "userId": 1
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| action | 是 | Integer | 动作：`0` 收藏，`1` 取消收藏 |
| renter_id | 是 | String | 租户 id，兼容 `renterId` |
| templateId | 是 | Long | 模板 id |
| userId | 是 | Long | 用户 id |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | `收藏成功` 或 `取消收藏成功` |
| data | Object | 空对象 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "收藏成功",
  "data": {}
}
```

## 4. AI剪辑终端绑定接口

### 4.1 查询用户绑定终端

**接口地址**：`/api/terminal/list`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：平台管理员可查询全部；租户管理员只能查询本租户；普通用户只能查询自己。

**请求示例**：

```json
{
  "active": 0,
  "renter_id": "default",
  "terminalUuid": "",
  "userId": 1
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| active | 否 | Integer | 是否生效：`0` 生效中，`1` 已解绑；不传默认查生效中 |
| renter_id | 否 | String | 租户 id，兼容 `renterId` |
| terminalUuid | 否 | String | 终端机器码 |
| userId | 否 | Long | 用户 id |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data | Array | 终端绑定列表 |
| data[].terminalId | Long | 终端绑定 id |
| data[].renterId | String | 租户 id |
| data[].userId | Long | 用户 id |
| data[].terminalUuid | String | 终端机器码 |
| data[].terminalName | String | 终端名称 |
| data[].terminalType | Integer | 终端类型 |
| data[].bindTime | Date | 绑定时间 |
| data[].lastLoginTime | Date | 最后登录时间 |
| data[].lastLoginIp | String | 最后登录 IP |
| data[].active | Integer | 是否生效：`0` 生效中，`1` 已解绑 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "查询成功",
  "data": [
    {
      "terminalId": 1,
      "renterId": "default",
      "userId": 1,
      "terminalUuid": "mac-001",
      "terminalName": "我的MacBook Pro",
      "terminalType": 1,
      "bindTime": "2026-06-02 10:00:00",
      "lastLoginTime": "2026-06-02 10:20:00",
      "lastLoginIp": "127.0.0.1",
      "active": 0
    }
  ]
}
```

### 4.2 校验用户是否还能绑定终端

**接口地址**：`/api/terminal/check`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：用于单独校验用户当前生效终端数量是否已达到上限。

**请求示例**：

```json
{
  "renter_id": "default",
  "terminalUuid": "mac-001",
  "userId": 1
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| renter_id | 否 | String | 租户 id，平台管理员校验指定用户时必传 |
| userId | 否 | Long | 用户 id，平台管理员或租户管理员校验指定用户时必传 |
| terminalUuid | 否 | String | 当前终端机器码 |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | `允许绑定` 或 `已达到绑定上限` |
| data.renterId | String | 租户 id |
| data.userId | Long | 用户 id |
| data.activeCount | Integer | 当前生效终端数 |
| data.maxActiveCount | Integer | 最大可绑定终端数 |
| data.boundCurrentTerminal | Boolean | 当前终端是否已绑定 |
| data.canBind | Boolean | 是否允许绑定 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "允许绑定",
  "data": {
    "renterId": "default",
    "userId": 1,
    "activeCount": 1,
    "maxActiveCount": 2,
    "boundCurrentTerminal": true,
    "canBind": true
  }
}
```

### 4.3 解绑用户终端

**接口地址**：`/api/terminal/unbind`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：平台管理员可解绑全部；租户管理员只能解绑本租户；普通用户只能解绑自己。

**请求示例**：

```json
{
  "terminalId": 1
}
```

也支持：

```json
{
  "renter_id": "default",
  "terminalUuid": "mac-001",
  "userId": 1
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| terminalId | 否 | Long | 终端绑定 id；传该字段时可不传其它定位字段 |
| renter_id | 条件必填 | String | 租户 id；不传 `terminalId` 时必填 |
| terminalUuid | 条件必填 | String | 终端机器码；不传 `terminalId` 时必填 |
| userId | 条件必填 | Long | 用户 id；不传 `terminalId` 时必填 |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data | Object | 空对象 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "解绑成功",
  "data": {}
}
```

## 5. AI剪辑账号接口

### 5.1 用户登录

**接口地址**：`/login`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：手机号、密码登录；手机号存在多个租户时返回租户列表，选择租户后重新调用本接口返回 token。

**请求示例**：

```json
{
  "account": "",
  "code": "",
  "password": "admin123",
  "phone": "15666666666",
  "renterId": "",
  "terminalName": "我的MacBook Pro",
  "terminalType": 1,
  "terminalUuid": "mac-001",
  "type": 0,
  "username": "15666666666",
  "uuid": ""
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| phone | 是 | String | 手机号。后端优先取 `phone`，为空时取 `username` |
| username | 否 | String | 兼容字段，可填手机号 |
| password | 是 | String | 密码 |
| renterId | 否 | String | 租户 id。首次登录可为空；多租户选择后再次登录时必填 |
| terminalUuid | 否 | String | 终端机器码，设备绑定使用 |
| terminalName | 否 | String | 终端名称 |
| terminalType | 否 | Integer | 终端类型 |
| account | 否 | String | 兼容字段 |
| code | 否 | String | 兼容字段 |
| type | 否 | Integer | 兼容字段 |
| uuid | 否 | String | 兼容字段 |

**登录业务结果码**：

| data.result | 说明 |
| --- | --- |
| 0 | 登录成功 |
| 1 | 用户存在多个租户下，需要前端展示租户列表 |
| 2 | 用户已绑定 2 个电脑，不返回 token |
| 3 | 其它异常，包括用户不存在、手机号格式错误、密码错误 |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 接口状态码 |
| msg | String | 登录结果描述 |
| data.result | Integer | 登录业务结果码 |
| data.renterList | Array | 租户列表 |
| data.renterList[].userId | Long | 用户 id |
| data.renterList[].renterId | String | 租户 id |
| data.renterList[].renterName | String | 租户名称 |
| data.renterList[].loginAccount | String | 登录手机号 |
| data.renterList[].account | String | 用户账号 |
| data.renterList[].passwordMatched | Boolean | 当前输入密码是否匹配该租户账号 |
| data.token | String | 登录成功时返回 token |

**多租户响应示例**：

```json
{
  "code": 0,
  "msg": "用户存在多个租户下，请选择租户",
  "data": {
    "result": 1,
    "renterList": [
      {
        "userId": 1,
        "renterId": "-1",
        "renterName": "系统租户",
        "loginAccount": "15666666666",
        "account": "admin",
        "passwordMatched": true
      }
    ]
  }
}
```

**登录成功响应示例**：

```json
{
  "code": 0,
  "msg": "登录成功",
  "data": {
    "result": 0,
    "renterList": [
      {
        "userId": 1,
        "renterId": "-1",
        "renterName": "系统租户",
        "loginAccount": "15666666666",
        "account": "admin",
        "passwordMatched": true
      }
    ],
    "token": "eyJhbGciOiJIUzUxMiJ9..."
  }
}
```

**密码错误响应示例**：

```json
{
  "code": 0,
  "msg": "密码错误",
  "data": {
    "result": 3,
    "renterList": []
  }
}
```

### 5.2 查询用户信息

**接口地址**：`/getInfo`

**请求方式**：`GET`

**请求数据类型**：`application/x-www-form-urlencoded`

**接口描述**：输入租户 id 和用户 id，返回用户信息、角色、权限；未传参数时从当前 token 读取。

**请求头**：

```http
Authorization: Bearer <token>
```

**请求参数**：

| 参数名称 | 请求类型 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- | --- |
| renterId | query | 否 | String | 租户 id。传入时查询指定用户 |
| userId | query | 否 | Long | 用户 id。传入时查询指定用户 |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data | Object | 当前为空对象 |
| user | Object | 用户对象 |
| user.userId | Long | 用户 id |
| user.userName | String | 用户账号 |
| user.nickName | String | 用户昵称 |
| user.phonenumber | String | 手机号 |
| user.renterId | String | 租户 id |
| user.creditBalance | Integer | 当前积分 |
| user.totalCreditGranted | Integer | 累计获得积分 |
| roles | Array | 角色标识集合 |
| permissions | Array | 权限标识集合 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "操作成功",
  "data": {},
  "user": {
    "userId": 1,
    "userName": "admin",
    "nickName": "Loogear",
    "phonenumber": "15666666666",
    "renterId": "-1",
    "creditBalance": 0,
    "totalCreditGranted": 0
  },
  "roles": [
    "admin"
  ],
  "permissions": [
    "*:*:*"
  ]
}
```

说明：当前代码未把 `renterName` 单独放入返回体；如前端需要租户名称，需要后续再补字段。

### 5.3 发送短信验证码

**接口地址**：`/resetPwd/sendCode`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：输入用户 id 和用户手机号，返回短信验证码。

**请求示例**：

```json
{
  "userId": 1,
  "phone": "15666666666"
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| userId | 否 | Long | 用户 id。传入时校验手机号必须属于该用户 |
| phone | 是 | String | 用户手机号 |
| phoneOrEmail | 否 | String | 兼容旧字段；`phone` 为空时使用 |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data.smsCode | String | 短信验证码 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "发送成功",
  "data": {
    "smsCode": "123456"
  }
}
```

**校验失败示例**：

```json
{
  "code": 1,
  "msg": "用户信息校验失败",
  "data": {}
}
```

### 5.4 修改密码

**接口地址**：`/resetPwd/reset`

**请求方式**：`POST`

**请求数据类型**：`application/json`

**接口描述**：支持旧密码修改和验证码修改两种方式。

**旧密码修改请求示例**：

```json
{
  "renterId": "-1",
  "userId": 1,
  "phone": "15666666666",
  "modifyType": 0,
  "oldPassword": "admin123",
  "newPassword": "newAdmin123"
}
```

**验证码修改请求示例**：

```json
{
  "renterId": "-1",
  "userId": 1,
  "phone": "15666666666",
  "modifyType": 1,
  "code": "123456",
  "newPassword": "newAdmin123"
}
```

**请求参数**：

| 参数名称 | 是否必须 | 数据类型 | 说明 |
| --- | --- | --- | --- |
| renterId | 是 | String | 租户 id |
| userId | 是 | Long | 用户 id |
| phone | 是 | String | 用户手机号 |
| modifyType | 是 | Integer | 修改方式：`0` 输入旧密码修改，`1` 输入验证码修改 |
| oldPassword | 条件必填 | String | 旧密码，`modifyType=0` 时必填 |
| code | 条件必填 | String | 短信验证码，`modifyType=1` 时必填 |
| newPassword | 是 | String | 新密码 |
| pwd | 否 | String | 兼容旧字段 |
| phoneOrEmail | 否 | String | 兼容旧字段 |

**响应参数**：

| 参数名称 | 数据类型 | 说明 |
| --- | --- | --- |
| code | Integer | 状态码 |
| msg | String | 结果描述 |
| data | Object | 空对象 |

**响应示例**：

```json
{
  "code": 0,
  "msg": "修改成功",
  "data": {}
}
```

**验证失败示例**：

```json
{
  "code": 1,
  "msg": "验证未通过",
  "data": {}
}
```

