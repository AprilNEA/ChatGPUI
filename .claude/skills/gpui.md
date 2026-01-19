# GPUI UI framework best practices for building desktop applications. Use when writing GPUI code, creating UI components, handling state/events, async tasks, animations, lists, forms, testing, or working with Zed-style Rust GUI code.

## 布局常见错误

### 1. `w_full() + margin` 导致溢出

**错误写法**：
```rust
// ❌ 子元素 w_full() + mx_2() = 100% + margin = 溢出
div().w_full().child(
    ListItem::new("item")
        .w_full()
        .mx_2()  // 这会导致溢出！
)
```

**正确写法**：
```rust
// ✅ 父容器用 padding 代替子元素 margin
div().w_full().px_2().child(
    ListItem::new("item")
        .w_full()
)
```

**原理**：CSS 盒模型中 `width: 100%` 不包含 margin，添加 margin 后总宽度超出容器。

### 2. VirtualList 固定高度

VirtualList 需要预计算每个 item 的尺寸：

```rust
// 定义固定高度常量
const ITEM_HEIGHT: f32 = 40.0;

// 构建尺寸数组
let sizes: Vec<Size<Pixels>> = items
    .iter()
    .map(|_| size(px(WIDTH), px(ITEM_HEIGHT)))
    .collect();

// 使用 VirtualList
v_virtual_list(
    cx.entity().clone(),
    "list-id",
    Rc::new(sizes),
    |view, visible_range, _, cx| {
        visible_range.map(|ix| /* render item */).collect()
    },
)
.w_full()
.flex_1()
.track_scroll(&scroll_handle)
```

### 3. 滚动容器必须有约束高度

```rust
// ❌ 错误：没有高度约束，无法滚动
v_flex()
    .overflow_y_scroll()
    .children(items)

// ✅ 正确：使用 flex_1() 或固定高度
v_flex()
    .flex_1()       // 或 .h(px(400.))
    .min_h_0()      // 重要：允许收缩
    .overflow_y_scroll()
    .children(items)
```

## 组件模式

### Entity 状态管理

```rust
pub struct MyComponent {
    state: SomeState,
}

impl MyComponent {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self { state: SomeState::default() }
    }

    pub fn update_state(&mut self, value: T, cx: &mut Context<Self>) {
        self.state = value;
        cx.notify();  // 通知 UI 重新渲染
    }
}

impl Render for MyComponent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 渲染逻辑
    }
}
```

### 事件发射

```rust
pub struct MyEvent {
    pub data: String,
}

impl EventEmitter<MyEvent> for MyComponent {}

// 发射事件
cx.emit(MyEvent { data: "value".to_string() });

// 订阅事件
cx.subscribe(&component, |this, _, event: &MyEvent, cx| {
    // 处理事件
});
```

### 异步任务

```rust
cx.spawn(async move |this, cx| {
    let result = async_operation().await;
    let _ = cx.update(|app| {
        let _ = this.update(app, |this, cx| {
            this.data = result;
            cx.notify();
        });
    });
}).detach();
```

## 主题使用

```rust
use gpui_component::{ActiveTheme, Theme, ThemeMode};

// 获取主题
let theme = cx.theme();

// 使用主题颜色
div()
    .bg(theme.background)
    .text_color(theme.foreground)
    .border_color(theme.border)

// 切换主题
Theme::change(ThemeMode::Dark, Some(window), cx);
```

## 常用布局

```rust
use gpui_component::{h_flex, v_flex};

// 水平布局
h_flex()
    .gap_2()
    .items_center()
    .justify_between()
    .children(items)

// 垂直布局
v_flex()
    .gap_4()
    .p_4()
    .children(items)

// 条件渲染
div()
    .when(condition, |this| this.child(element))
    .when_some(option_value, |this, value| this.child(value))
```
