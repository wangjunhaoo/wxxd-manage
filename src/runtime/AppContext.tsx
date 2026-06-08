/* ============================================================================
   App 状态上下文 —— 把移植后的 useWxXdApp（单例 store）接入 React
   ----------------------------------------------------------------------------
   - 整个应用共享同一个 ctx 实例（原 Vue 也是单 composable 实例）。
   - useApp() 通过 useSyncExternalStore 订阅响应式版本号：任意 ref/reactive 写入
     都会触发使用方重渲染，再读取最新 .value。
   - 挂载时执行 onMounted 钩子（即 refreshAll 首屏加载）。
   ============================================================================ */
import {
  createContext,
  useContext,
  useEffect,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { subscribe, getVersion, flushLifecycle } from "./reactive";
import { useWxXdApp, type WxXdAppContext } from "../composables/useWxXdApp";

// 单例：在模块加载时构造一次。useWxXdApp 内部使用的是本项目的响应式 shim
// （非 React Hooks），因此在组件树之外调用是安全的。
const store: WxXdAppContext = useWxXdApp();

const AppCtx = createContext<WxXdAppContext>(store);

export function AppProvider({ children }: { children: ReactNode }) {
  useEffect(() => {
    // 运行 onMounted（refreshAll 首屏加载），返回的清理函数执行 onUnmounted。
    return flushLifecycle();
  }, []);
  return <AppCtx.Provider value={store}>{children}</AppCtx.Provider>;
}

/** 订阅全局响应式版本号并返回共享 ctx。组件读取的任何 .value 变化都会触发重渲染。 */
export function useApp(): WxXdAppContext {
  useSyncExternalStore(subscribe, getVersion, getVersion);
  return useContext(AppCtx);
}
