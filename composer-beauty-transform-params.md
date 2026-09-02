# Composer 美颜预览视频变换参数约定

## 1. 适用接口

本文档适用于以下两个接口的 `json_params` 参数：

```c
int composer_beauty_process_frame(const char* input_video_path,
                                  int64_t timestamp_ms,
                                  const char* output_image_path,
                                  const char* json_params);

int composer_beauty_process_file(const char* input_path,
                                 const char* output_path,
                                 int64_t start_time_ms,
                                 int64_t duration_ms,
                                 const char* json_params);
```

图片预览和视频预览必须使用相同的变换规则。

## 2. 完整 JSON 示例

```json
{
  "whiteness": 0.5,
  "smoothing": 0.3,
  "saturation": 100.0,
  "skin_tone": 0.0,
  "face_detect": 1,
  "rotation": 30.0,
  "lut_file": "/absolute/path/to/lut.cube",
  "lut_intensity": 0.8,
  "positionX": 480.0,
  "positionY": 270.0,
  "scale": 0.5,
  "canvas_width": 960,
  "canvas_height": 540,
  "transform_origin": "center",
  "rotation_direction": "clockwise",
  "stabilization": false,
  "one_click_beauty": false
}
```

字段名区分大小写。`positionX` 和 `positionY` 使用驼峰命名，其他新增字段使用下划线命名。

`saturation` 的取值范围为 `0`～`200`，其中 `100` 表示保持原始饱和度，`0` 表示完全去色，超过 `100` 表示增强饱和度。

## 3. 变换字段

| 字段 | JSON 类型 | 当前值或范围 | 说明 |
| --- | --- | --- | --- |
| `positionX` | number | 不限制 | 视频中心点在前端预览画布中的 X 坐标，单位为预览画布像素 |
| `positionY` | number | 不限制 | 视频中心点在前端预览画布中的 Y 坐标，单位为预览画布像素 |
| `scale` | number | `0.01`～`10.0` | Fabric 视频对象相对于源视频像素的等比缩放系数，不是百分数 |
| `rotation` | number | 不限制 | 围绕视频中心旋转的累计角度，单位为度；正数表示顺时针，负数表示逆时针，可超过一圈 |
| `canvas_width` | integer | 当前固定为 `960` | 产生 `positionX`、`scale` 等数据的前端预览画布宽度 |
| `canvas_height` | integer | 当前固定为 `540` | 产生 `positionY`、`scale` 等数据的前端预览画布高度 |
| `transform_origin` | string | 当前固定为 `center` | 位置、缩放和旋转均以源视频中心为变换原点 |
| `rotation_direction` | string | `clockwise` / `counterclockwise` | 根据 `rotation` 的正负生成的方向说明；非负值为 `clockwise`，负值为 `counterclockwise` |

前端属性面板中显示的缩放百分数等于 `scale * 100`。例如面板显示 `50%` 时，JSON 中的 `scale` 为 `0.5`。

`positionX`、`positionY` 允许超出画布范围或为负数，以支持把视频的一部分移动到画布外。

## 4. 坐标系

前端使用左上角为画布原点：

```text
(0, 0) --------------------> X
  |
  |
  |
  v
  Y
```

`positionX`、`positionY` 指向视频中心，不是视频左上角。默认居中位置为：

```text
positionX = canvas_width / 2
positionY = canvas_height / 2
```

当前默认值为 `(480, 270)`。

## 5. C++ 还原公式

设：

- 前端预览画布尺寸为 `canvas_width × canvas_height`。
- C++ 最终输出画布尺寸为 `output_width × output_height`。
- 解码后的源视频尺寸为 `source_width × source_height`。

先计算坐标换算比例：

```text
ratio_x = output_width  / canvas_width
ratio_y = output_height / canvas_height
```

前端和 C++ 输出画布宽高比一致时，`ratio_x` 应等于 `ratio_y`。此时：

```text
center_x = positionX * ratio_x
center_y = positionY * ratio_y
render_scale = scale * ratio_x
```

变换后的源视频显示尺寸为：

```text
render_width  = source_width  * render_scale
render_height = source_height * render_scale
```

必须按照以下顺序应用变换：

```text
1. 将源视频中心移动到局部坐标原点
2. 按 render_scale 等比缩放
3. 围绕中心按 rotation 旋转：正值顺时针，负值逆时针
4. 将旋转后的中心移动到 center_x、center_y
5. 裁剪到 output_width × output_height 输出画布
```

如果使用的图像库规定正角度为逆时针，例如常见的 OpenCV 旋转接口，则传给该接口的角度应取负数：

```cpp
double library_angle = -json_rotation;
```

## 6. 计算示例

输入视频和 C++ 输出画布均为 `1920×1080`，前端画布为 `960×540`：

```json
{
  "positionX": 480,
  "positionY": 270,
  "scale": 0.5,
  "rotation": 30,
  "canvas_width": 960,
  "canvas_height": 540,
  "transform_origin": "center",
  "rotation_direction": "clockwise"
}
```

换算结果：

```text
ratio_x = 1920 / 960 = 2
ratio_y = 1080 / 540 = 2

center_x = 480 * 2 = 960
center_y = 270 * 2 = 540
render_scale = 0.5 * 2 = 1.0
```

因此 C++ 应把源视频保持原始尺寸，以 `(960, 540)` 为中心，顺时针旋转 30 度，然后裁剪到 `1920×1080`。

## 7. 输出宽高比要求

要和前端预览完全一致，C++ 输出画布宽高比必须与 `canvas_width / canvas_height` 一致。当前为 `16:9`。

如果输出画布不是 `16:9`，`ratio_x` 和 `ratio_y` 会不同。此时不能同时保持等比缩放和完全一致的 X/Y 映射，C++ 必须与前端另行约定裁剪或留黑规则。当前版本不支持这种情况。

源视频宽高比可以不是 `16:9`；只要求最终输出画布与前端预览画布宽高比一致。

## 8. 默认值与兼容处理

Tauri 层对缺失参数使用以下默认值：

```json
{
  "positionX": 0,
  "positionY": 0,
  "scale": 1,
  "rotation": 0,
  "canvas_width": 960,
  "canvas_height": 540,
  "transform_origin": "center",
  "rotation_direction": "clockwise"
}
```

其中 `transform_origin` 当前只接受 `center`；`rotation_direction` 接受 `clockwise` 和 `counterclockwise`。`rotation` 保存用户的累计圈数，例如 `900` 表示顺时针两圈半，`-540` 表示逆时针一圈半。C++ 渲染时可以使用与其等效的模 360 角度，但不得把归一化结果当成持久化值回写。旋转方向应以 `rotation` 的符号为准，`rotation_direction` 用于校验和调试展示。

前端将 `rotation` 保留到小数点后两位。

`source_width` 和 `source_height` 不通过 JSON 传递，由 C++ 在解码输入视频后获取。

## 9. 工程模板 XML 中的属性

用户调整属性后，应用会根据当前素材的 `asset id`，更新当前工程 `template.xml` 中所有 `asset-id` 相同的 `area`。`property` 位于 `area` 的最后，重复调整时更新原节点，不会重复追加：

```xml
<area id="area-id" asset-id="asset-id">
    <!-- area 原有节点 -->
    <property>
        <whiteness>0.0</whiteness>
        <smoothing>0.0</smoothing>
        <saturation>122.0</saturation>
        <skin_tone>0.0</skin_tone>
        <face_detect>1</face_detect>
        <rotation>0.0</rotation>
        <lut_style>lut-009</lut_style>
        <lut_intensity>0.0</lut_intensity>
        <positionX>480.0</positionX>
        <positionY>270.0</positionY>
        <scale>0.5</scale>
        <canvas_width>960</canvas_width>
        <canvas_height>540</canvas_height>
        <transform_origin>center</transform_origin>
        <rotation_direction>clockwise</rotation_direction>
        <stabilization>false</stabilization>
        <one_click_beauty>false</one_click_beauty>
    </property>
</area>
```

`lut_style` 保存 `src-tauri/resources/luts/luts.json` 中对应选项的 `id`，例如 `lut-009`；未选择 LUT 时保存 `none`。应用重新打开原始模板或工程草稿时根据该 ID 恢复下拉框，再在运行时解析当前安装目录中的 LUT 绝对路径。`lut_style` 只用于模板持久化，不传给 C++ 美颜接口；C++ 仍读取 JSON 中的 `lut_file` 和 `lut_intensity`。
