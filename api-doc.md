# HTTP 接口文档（Node）

## 1. 文档说明

本文档描述剪辑系统相关 HTTP 接口，包括用户登录、用户信息、剪辑模板、工程管理、视频导出、短信验证码与密码修改等能力。

## 2. 通用约定

### 2.1 请求方式

除特殊说明外，接口建议使用：

- 请求方法：`POST`
- 请求格式：`application/json`
- 响应格式：`application/json`

### 2.2 通用响应格式

```json
{
  "code": 0,
  "message": "成功",
  "data": {}
}
```

字段说明：

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| code | number | 是 | 执行结果码，具体含义见各接口说明 |
| message | string | 是 | 结果描述 |
| data | object / array / null | 否 | 响应数据 |

---

## API-1 用户登录

### 接口地址

`POST /api/user/login`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| phone | string | 是 | 手机号 |
| password | string | 是 | 密码 |
| tenantId | string | 否 | 租户 ID |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| result | number | 登录结果：`0` 成功，`1` 用户存在多个租户下，`2` 用户已绑定 2 个电脑，`3` 其它异常 |
| message | string | 结果描述 |
| tenantList | array | 租户列表 |

tenantList 字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| userId | string | 用户 ID |
| tenantId | string | 租户 ID |
| tenantName | string | 租户名称 |
| passwordMatched | boolean | 密码是否匹配 |

### 响应示例

```json
{
  "result": 0,
  "message": "登录成功",
  "tenantList": [
    {
      "userId": "10001",
      "tenantId": "t001",
      "tenantName": "默认租户",
      "passwordMatched": true
    }
  ]
}
```

---

## API-2 查询用户信息

### 接口地址

`POST /api/user/info`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| tenantId | string | 是 | 租户 ID |
| userId | string | 是 | 用户 ID |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| phone | string | 用户电话号码 |
| nickname | string | 昵称 |
| tenantId | string | 所属租户 ID |
| tenantName | string | 所属租户名称 |
| points | number | 当前积分 |

### 响应示例

```json
{
  "phone": "13800138000",
  "nickname": "张三",
  "tenantId": "t001",
  "tenantName": "默认租户",
  "points": 100
}
```

---

## API-3 查询剪辑模板类别

### 接口地址

`POST /api/template/categories`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| tenantId | string | 否 | 租户 ID，备用字段 |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| categoryId | string | 模板类别 ID |
| categoryName | string | 类别名称 |

### 响应示例

```json
[
  {
    "categoryId": "c001",
    "categoryName": "宣传片"
  }
]
```

---

## API-4 查询我的工程

### 接口地址

`POST /api/project/my`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| tenantId | string | 是 | 租户 ID |
| userId | string | 是 | 用户 ID |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| projectId | string | 工程 ID |
| projectName | string | 工程名称 |
| status | string | 状态：`编辑中`、`已导出` |

### 响应示例

```json
[
  {
    "projectId": "p001",
    "projectName": "我的工程",
    "status": "编辑中"
  }
]
```

---

## API-5 获取工程信息

### 接口地址

`POST /api/project/detail`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| projectId | string | 是 | 工程 ID |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| projectId | string | 工程 ID |
| projectName | string | 工程名称 |
| status | string | 状态：`编辑中`、`已导出` |
| exportPoints | number | 导出扣减积分值，预留字段 |
| templateId | string | 对应模板 ID |
| projectFileUrl | string | 工程文件地址 |

### 工程文件内容说明

工程文件中包含：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| styleList | array | 风格列表 |
| materialMappingList | array | 素材填充关系 |

styleList 字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| styleId | string | 风格 ID |
| requiredMaterials | array | 所需素材 |
| selectedMaterials | array | 已选择素材列表 |

selectedMaterials 字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| materialId | string | 素材 ID |
| path | string | 素材路径 |

materialMappingList 字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| templateMaterialId | string | 模板中素材 ID |
| selectedMaterialId | string | 选择的素材 ID |
| offsetSeconds | number | 素材偏移秒数 |

### 响应示例

```json
{
  "projectId": "p001",
  "projectName": "我的工程",
  "status": "编辑中",
  "exportPoints": 10,
  "templateId": "tpl001",
  "projectFileUrl": "https://example.com/projects/p001.xml"
}
```

---

## API-6 查询剪辑模板

### 接口地址

`POST /api/templates`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| tenantId | string | 否 | 租户 ID，备用字段 |
| userId | string | 是 | 用户 ID |
| categoryId | string | 否 | 模板类别 ID |
| keyword | string | 否 | 模板关键字 |
| favoriteOnly | boolean | 否 | 是否只查询收藏模板 |
| sortType | number | 否 | 排序方式：`0` 按模板名称，`1` 按创建时间倒序 |

### 查询规则

- 只返回状态为启用的模板。
- 按关键字搜索时，搜索模板名称和标签，任一项包含关键字则视为匹配。

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| templateId | string | 模板 ID |
| templateName | string | 模板名称 |
| thumbnailUrl | string | 模板缩略图地址，MinIO 位置 |
| clipCount | number | 包含片段数 |
| duration | number | 模板时长，单位秒 |

### 响应示例

```json
[
  {
    "templateId": "tpl001",
    "templateName": "企业宣传模板",
    "thumbnailUrl": "https://minio.example.com/templates/tpl001.png",
    "clipCount": 8,
    "duration": 60
  }
]
```

---

## API-7 获取剪辑模板详情

### 接口地址

`POST /api/template/detail`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| templateId | string | 是 | 模板 ID |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| templateId | string | 模板 ID |
| version | string | 版本号 |
| templateFileUrl | string | 模板文件下载地址，XML 格式 |
| materialPackageUrl | string | 模板素材压缩包下载地址 |

### 响应示例

```json
{
  "templateId": "tpl001",
  "version": "1.0.0",
  "templateFileUrl": "https://example.com/templates/tpl001.xml",
  "materialPackageUrl": "https://example.com/templates/tpl001.zip"
}
```

---

## API-8 收藏/取消收藏

### 接口地址

`POST /api/template/favorite`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| templateId | string | 是 | 模板 ID |
| tenantId | string | 是 | 租户 ID |
| userId | string | 是 | 用户 ID |
| action | number | 是 | 动作：`0` 收藏，`1` 取消收藏 |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| result | number | 执行结果：`0` 成功，`1` 异常 |
| message | string | 结果描述 |

### 响应示例

```json
{
  "result": 0,
  "message": "操作成功"
}
```

---

## API-9 新建工程基本信息

### 接口地址

`POST /api/project/create`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| projectName | string | 是 | 工程名称 |
| templateId | string | 是 | 模板 ID |
| tenantId | string | 是 | 租户 ID |
| userId | string | 是 | 用户 ID |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| result | number | 执行结果：`0` 成功，`1` 异常 |
| message | string | 结果描述 |
| projectId | string | 工程 ID |

### 响应示例

```json
{
  "result": 0,
  "message": "创建成功",
  "projectId": "p001"
}
```

---

## API-10 更新工程信息

### 接口地址

`POST /api/project/update`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| projectId | string | 是 | 工程 ID |
| templateId | string | 是 | 模板 ID |
| projectName | string | 是 | 工程名称 |
| projectXmlFile | file / string | 是 | 工程 XML 文件，可为文件上传或文件地址 |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| result | number | 执行结果：`0` 成功，`1` 异常 |
| message | string | 结果描述 |

### 响应示例

```json
{
  "result": 0,
  "message": "更新成功"
}
```

---

## API-11 从工程导出视频

### 接口地址

`POST /api/project/export`

### 接口说明

从工程导出视频时扣减用户积分，并更新工程状态。

成功扣减积分后，将工程状态更新为 `已导出`。

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| userId | string | 是 | 用户 ID |
| projectId | string | 是 | 工程 ID |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| result | number | 扣减结果：`0` 成功，`1` 积分不足，`2` 其它异常 |
| message | string | 结果描述 |

### 响应示例

```json
{
  "result": 0,
  "message": "导出成功"
}
```

---

## API-12 上传结果视频

### 接口地址

`POST /api/project/upload-result`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| userId | string | 是 | 用户 ID |
| projectId | string | 是 | 工程 ID |
| videoLocalPath | string | 是 | 结果视频本地路径 |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| result | number | 上传结果：`0` 成功，`1` 异常 |
| message | string | 结果描述 |

### 响应示例

```json
{
  "result": 0,
  "message": "上传成功"
}
```

---

## API-13 发送短信验证码

### 接口地址

`POST /api/sms/send-code`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| userId | string | 是 | 用户 ID |
| phone | string | 是 | 用户手机号 |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| result | number | 发送结果：`0` 成功，`1` 其它异常 |
| smsCode | string | 短信验证码 |
| message | string | 结果描述 |

### 响应示例

```json
{
  "result": 0,
  "smsCode": "123456",
  "message": "发送成功"
}
```

---

## API-14 修改密码

### 接口地址

`POST /api/user/change-password`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| tenantId | string | 是 | 租户 ID |
| userId | string | 是 | 用户 ID |
| changeType | number | 是 | 修改方式：`0` 输入旧密码修改，`1` 输入验证码修改 |
| oldPassword | string | 否 | 旧密码，changeType 为 `0` 时必填 |
| smsCode | string | 否 | 验证码，changeType 为 `1` 时必填 |
| newPassword | string | 是 | 新密码 |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| result | number | 修改结果：`0` 成功，`1` 验证未通过，`2` 其它异常 |
| message | string | 结果描述 |

### 响应示例

```json
{
  "result": 0,
  "message": "修改成功"
}
```

---

## API-15 退出登录

### 接口地址

`POST /api/user/logout`

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| tenantId | string | 是 | 租户 ID |
| userId | string | 是 | 用户 ID |

### 响应参数

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| result | number | 执行结果：`0` 成功，`1` 异常 |
| message | string | 结果描述 |

### 响应示例

```json
{
  "result": 0,
  "message": "退出成功"
}
```

