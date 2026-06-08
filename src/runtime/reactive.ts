/* ============================================================================
   轻量响应式 shim —— 替代 Vue 的 ref / computed / reactive
   ----------------------------------------------------------------------------
   目标：让原 Vue composable（useWxXdApp 及子 composable）几乎原样移植到 React，
   同时彻底移除对 vue 包的依赖。

   设计：
   - 全局单一版本号 + 监听器集合。任何 ref/reactive 的写入都会「同步自增版本号」
     并「微任务批量」通知订阅者。
   - React 组件通过 useReactiveStore() 经 useSyncExternalStore 订阅版本号，
     版本变化即重渲染并读取最新 .value（粗粒度、整树重渲，内部工具场景完全够用）。
   - computed 按版本号记忆：同一版本内只计算一次。
   这套语义与 Vue 的差异仅在「无细粒度依赖追踪」「computed 不做依赖级缓存」，
   对一个双人内部控制台而言无感知。
   ============================================================================ */

export interface Ref<T> {
  value: T;
}
export interface ComputedRef<T> {
  readonly value: T;
}
export interface WritableComputedRef<T> {
  value: T;
}

// ---- 全局通知中枢 ---------------------------------------------------------
// 两个计数器：
//  - version：同步自增，供 computed 记忆与「写后立即读」在同一 tick 内一致。
//  - snapshotVersion：仅在「微任务批量」里同步到 version，作为 useSyncExternalStore
//    的 getSnapshot 返回值。这样渲染期间发生的写入（如某个 section 在渲染时初始化
//    默认值）不会立刻改变快照 → 不会触发 useSyncExternalStore 的同步重渲染死循环；
//    它只会调度一次微任务，渲染结束后干净地重渲一次（语义等同 setState）。
let version = 0;
let snapshotVersion = 0;
let flushScheduled = false;
const listeners = new Set<() => void>();

function scheduleNotify(): void {
  version++;
  if (flushScheduled) return;
  flushScheduled = true;
  queueMicrotask(() => {
    flushScheduled = false;
    snapshotVersion = version;
    listeners.forEach((l) => l());
  });
}

export function getVersion(): number {
  return snapshotVersion;
}

export function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** 手动触发一次通知（用于无法走 setter 的就地变更场景）。 */
export function touch(): void {
  scheduleNotify();
}

// ---- reactive ：对象/数组的深层代理，任意写入都会通知 -----------------------
const RAW = Symbol("raw");
const proxyCache = new WeakMap<object, any>();

function isPlainReactiveTarget(value: unknown): value is object {
  if (value === null || typeof value !== "object") return false;
  if ((value as any)[RAW]) return true; // 已是代理
  const tag = Object.prototype.toString.call(value);
  return tag === "[object Object]" || tag === "[object Array]";
}

export function reactive<T extends object>(target: T): T {
  if (!isPlainReactiveTarget(target)) return target;
  if ((target as any)[RAW]) return target; // 已经是代理
  const existing = proxyCache.get(target);
  if (existing) return existing;

  const proxy = new Proxy(target as any, {
    get(obj, key, receiver) {
      if (key === RAW) return true;
      const result = Reflect.get(obj, key, receiver);
      // 深层包裹，确保嵌套写入同样可被侦测
      if (isPlainReactiveTarget(result)) return reactive(result);
      return result;
    },
    set(obj, key, value, receiver) {
      const had = Object.prototype.hasOwnProperty.call(obj, key);
      const old = (obj as any)[key];
      const next = isPlainReactiveTarget(value) ? (value as any) : value;
      const ok = Reflect.set(obj, key, next, receiver);
      if (!had || !Object.is(old, next)) scheduleNotify();
      return ok;
    },
    deleteProperty(obj, key) {
      const had = Object.prototype.hasOwnProperty.call(obj, key);
      const ok = Reflect.deleteProperty(obj, key);
      if (had) scheduleNotify();
      return ok;
    },
  });
  proxyCache.set(target, proxy);
  return proxy;
}

// ---- ref ：标量/对象通用容器，写入即通知 ----------------------------------
class RefImpl<T> {
  private _raw: T;
  constructor(value: T) {
    this._raw = value;
  }
  get value(): T {
    // 对象型 ref 读取时返回 reactive 代理，使 `ref.value.field = x` 也能通知
    if (isPlainReactiveTarget(this._raw)) return reactive(this._raw as any);
    return this._raw;
  }
  set value(v: T) {
    if (Object.is(v, this._raw)) return;
    this._raw = v;
    scheduleNotify();
  }
}

export function ref<T>(value: T): Ref<T>;
export function ref<T = any>(): Ref<T | undefined>;
export function ref(value?: unknown) {
  return new RefImpl(value);
}

export function isRef<T>(r: any): r is Ref<T> {
  return r instanceof RefImpl;
}

export function unref<T>(r: Ref<T> | T): T {
  return isRef(r) ? r.value : (r as T);
}

// ---- computed ：按版本号记忆 ---------------------------------------------
class ComputedImpl<T> {
  private _cache!: T;
  private _ver = -1;
  constructor(
    private getter: () => T,
    private setter?: (v: T) => void,
  ) {}
  get value(): T {
    if (this._ver !== version) {
      this._cache = this.getter();
      this._ver = version;
    }
    return this._cache;
  }
  set value(v: T) {
    if (this.setter) this.setter(v);
    else if (import.meta.env?.DEV)
      console.warn("[reactive] 对只读 computed 赋值被忽略");
  }
}

export function computed<T>(getter: () => T): ComputedRef<T>;
export function computed<T>(options: {
  get: () => T;
  set: (v: T) => void;
}): WritableComputedRef<T>;
export function computed<T>(
  arg: (() => T) | { get: () => T; set: (v: T) => void },
): any {
  if (typeof arg === "function") return new ComputedImpl(arg);
  return new ComputedImpl(arg.get, arg.set);
}

// ---- watch ：基于全局通知的简化实现 --------------------------------------
type WatchSource<T> = Ref<T> | ComputedRef<T> | (() => T);
interface WatchOptions {
  immediate?: boolean;
  deep?: boolean;
}

function evalSource<T>(source: WatchSource<T>): T {
  if (typeof source === "function") return (source as () => T)();
  return (source as Ref<T>).value;
}

export function watch<T>(
  source: WatchSource<T>,
  cb: (value: T, oldValue: T | undefined) => void,
  options: WatchOptions = {},
): () => void {
  let prev: T | undefined = options.immediate ? undefined : evalSource(source);
  if (options.immediate) {
    const initial = evalSource(source);
    prev = initial;
    cb(initial, undefined);
  }
  return subscribe(() => {
    const next = evalSource(source);
    if (!Object.is(next, prev)) {
      const old = prev;
      prev = next;
      cb(next, old);
    }
  });
}

// ---- 生命周期钩子（单例 store 用）----------------------------------------
const mountedHooks: Array<() => void> = [];
const unmountedHooks: Array<() => void> = [];

export function onMounted(cb: () => void): void {
  mountedHooks.push(cb);
}
export function onUnmounted(cb: () => void): void {
  unmountedHooks.push(cb);
}

/** 由 AppProvider 在挂载时调用：执行所有 onMounted，返回清理函数。 */
export function flushLifecycle(): () => void {
  const hooks = mountedHooks.splice(0);
  hooks.forEach((h) => h());
  return () => {
    const cleanups = unmountedHooks.splice(0);
    cleanups.forEach((c) => c());
  };
}

// ---- nextTick ：微任务对齐 ------------------------------------------------
export function nextTick(cb?: () => void): Promise<void> {
  return Promise.resolve().then(() => {
    if (cb) cb();
  });
}
