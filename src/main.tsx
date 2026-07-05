// React 应用入口
//
// 职责：
// 1. 创建 React 根节点，渲染根组件 <App/>
// 2. 加载全局样式表 src/styles/global.css —— 包含所有 design tokens、基础组件样式
// 3. 启用 React StrictMode（开发期检查副作用、不影响生产构建）

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/global.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
