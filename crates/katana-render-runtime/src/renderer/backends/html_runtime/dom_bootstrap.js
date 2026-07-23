const __krrNativeDom = globalThis.__krr_dom;
class __KrrEvent {
  constructor(...args) {
    if (args.length === 0) throw new TypeError("Event type must be provided");
    const [type, options = {}] = args;
    const normalizedOptions = options ?? {};
    this.type = String(type);
    this.bubbles = Boolean(normalizedOptions.bubbles);
    this.cancelable = Boolean(normalizedOptions.cancelable);
    this.composed = Boolean(normalizedOptions.composed);
    this.defaultPrevented = false;
    this.target = null;
    this.currentTarget = null;
    this.eventPhase = 0;
    this.isTrusted = false;
    this.timeStamp = Date.now();
    this.__krrDispatching = false;
    this.__krrPropagationStopped = false;
    this.__krrImmediatePropagationStopped = false;
    this.__krrPassiveListener = false;
  }
  preventDefault() {
    if (this.cancelable && !this.__krrPassiveListener) this.defaultPrevented = true;
  }
  stopPropagation() {
    this.__krrPropagationStopped = true;
  }
  stopImmediatePropagation() {
    this.__krrPropagationStopped = true;
    this.__krrImmediatePropagationStopped = true;
  }
}
globalThis.Event = __KrrEvent;
Object.assign(__KrrEvent, {
  NONE: 0,
  CAPTURING_PHASE: 1,
  AT_TARGET: 2,
  BUBBLING_PHASE: 3,
});
Object.assign(__KrrEvent.prototype, {
  NONE: 0,
  CAPTURING_PHASE: 1,
  AT_TARGET: 2,
  BUBBLING_PHASE: 3,
});
const __krrListenerOptions = (options) => ({
  capture: typeof options === "boolean" ? options : Boolean(options?.capture),
  once: Boolean(typeof options === "object" && options?.once),
  passive: Boolean(typeof options === "object" && options?.passive),
});
const __krrEventTargetListeners = new WeakMap();
const __krrEventHandlers = new WeakMap();
const __krrSyncEventTarget = (target, type, entries) => {
  if (!target.__krrNodeId) return;
  const handler = __krrEventHandlers.get(target)?.get(String(type));
  __krrNativeDom(
    "setEventTarget",
    target.__krrNodeId,
    String(type),
    String(entries.length > 0 || typeof handler === "function"),
  );
};
const __krrDispatchListeners = (listeners, target, event, capture) => {
  const entries = listeners.get(String(event.type)) || [];
  for (const entry of [...entries]) {
    if (!entries.includes(entry)) continue;
    if (entry.capture !== capture) continue;
    if (entry.once) entries.splice(entries.indexOf(entry), 1);
    event.__krrPassiveListener = entry.passive;
    try {
      if (typeof entry.callback === "function") entry.callback.call(target, event);
      else entry.callback.handleEvent.call(entry.callback, event);
    } finally {
      event.__krrPassiveListener = false;
    }
    if (event.__krrImmediatePropagationStopped) break;
  }
  __krrSyncEventTarget(target, event.type, entries);
};
const __krrDispatchHandler = (target, event) => {
  if (event.__krrImmediatePropagationStopped) return;
  const handler = target[`on${event.type}`];
  if (typeof handler !== "function") return;
  const result = handler.call(target, event);
  if (result === false && event.cancelable) event.preventDefault();
};
const __krrBeginDispatch = (target, event) => {
  if (!event || typeof event !== "object" || !event.type) throw new TypeError("Invalid event");
  if (event.__krrDispatching) throw new Error("Event is already being dispatched");
  event.__krrDispatching = true;
  event.__krrPropagationStopped = false;
  event.__krrImmediatePropagationStopped = false;
  event.target = target;
  event.currentTarget = target;
  event.eventPhase = 2;
};
const __krrEndDispatch = (event) => {
  event.currentTarget = null;
  event.eventPhase = 0;
  event.__krrDispatching = false;
};
const __krrDispatchTargetPhase = (target, event, capture, phase) => {
  event.currentTarget = target;
  event.eventPhase = phase;
  const listeners = __krrEventTargetListeners.get(target) || new Map();
  __krrDispatchListeners(listeners, target, event, capture);
  if (!capture) __krrDispatchHandler(target, event);
};
const __krrInstallEventTarget = (target) => {
  const listeners = new Map();
  __krrEventTargetListeners.set(target, listeners);
  Object.defineProperties(target, {
    addEventListener: {
      configurable: true,
      value(type, callback, options) {
        if (callback === null || callback === undefined) return;
        const callable =
          typeof callback === "function" ||
          (typeof callback === "object" && typeof callback.handleEvent === "function");
        if (!callable)
          throw new TypeError("Event listener must be a function or EventListener object");
        type = String(type);
        const normalized = __krrListenerOptions(options);
        const entries = listeners.get(type) || [];
        if (
          !entries.some(
            (entry) => entry.callback === callback && entry.capture === normalized.capture,
          )
        ) {
          entries.push({
            callback,
            capture: normalized.capture,
            once: normalized.once,
            passive: normalized.passive,
          });
          listeners.set(type, entries);
        }
        __krrSyncEventTarget(target, type, entries);
      },
    },
    removeEventListener: {
      configurable: true,
      value(type, callback, options) {
        const entries = listeners.get(String(type));
        if (!entries) return;
        const { capture } = __krrListenerOptions(options);
        const index = entries.findIndex(
          (entry) => entry.callback === callback && entry.capture === capture,
        );
        if (index >= 0) entries.splice(index, 1);
        __krrSyncEventTarget(target, type, entries);
      },
    },
    dispatchEvent: {
      configurable: true,
      value(event) {
        if (target.__krrNodeId) {
          const dispatched = __krrDispatchElementEvent(target, event);
          return !dispatched.defaultPrevented;
        }
        __krrBeginDispatch(target, event);
        try {
          __krrDispatchTargetPhase(target, event, true, Event.AT_TARGET);
          if (!event.__krrImmediatePropagationStopped)
            __krrDispatchTargetPhase(target, event, false, Event.AT_TARGET);
          return !event.defaultPrevented;
        } finally {
          __krrEndDispatch(event);
        }
      },
    },
  });
  return target;
};
const __krrDatasetAttribute = (property) =>
  `data-${String(property).replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`;
const __krrClassToken = (token) => {
  token = String(token);
  if (!token || /\s/.test(token)) throw new TypeError("Invalid class token");
  return token;
};
const __krrElements = new Map();
const __krrElement = (nodeId) => {
  if (nodeId === null || nodeId === undefined || nodeId === "") return null;
  const normalizedId = String(nodeId);
  const cached = __krrElements.get(normalizedId);
  if (cached) return cached;
  const element = __krrInstallEventTarget(Object.create(__krrElementPrototype));
  Object.defineProperty(element, "__krrNodeId", { value: normalizedId });
  __krrElements.set(normalizedId, element);
  return element;
};
const __krrElementPrototype = {
  get onclick() {
    return __krrEventHandlers.get(this)?.get("click") ?? null;
  },
  set onclick(value) {
    let handlers = __krrEventHandlers.get(this);
    if (!handlers) {
      handlers = new Map();
      __krrEventHandlers.set(this, handlers);
    }
    if (value === null || value === undefined) handlers.delete("click");
    else handlers.set("click", value);
    const listeners = __krrEventTargetListeners.get(this) || new Map();
    __krrSyncEventTarget(this, "click", listeners.get("click") || []);
  },
  get textContent() {
    return __krrNativeDom("textContent", this.__krrNodeId);
  },
  set textContent(value) {
    __krrNativeDom("setTextContent", this.__krrNodeId, String(value));
  },
  get innerHTML() {
    return __krrNativeDom("innerHTML", this.__krrNodeId);
  },
  set innerHTML(value) {
    __krrNativeDom("setInnerHTML", this.__krrNodeId, String(value));
  },
  get className() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "class") || "";
  },
  set className(value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, "class", String(value));
  },
  get classList() {
    const read = () => new Set(this.className.split(/\s+/).filter(Boolean));
    const write = (tokens) => {
      const value = Array.from(tokens).join(" ");
      if (value) this.className = value;
      else this.removeAttribute("class");
    };
    return {
      add(...values) {
        const tokens = read();
        for (const value of values) tokens.add(__krrClassToken(value));
        write(tokens);
      },
      remove(...values) {
        const tokens = read();
        for (const value of values) tokens.delete(__krrClassToken(value));
        write(tokens);
      },
      contains(value) {
        return read().has(__krrClassToken(value));
      },
      toggle(value, force) {
        const token = __krrClassToken(value);
        const tokens = read();
        const enabled = force === undefined ? !tokens.has(token) : Boolean(force);
        if (enabled) tokens.add(token);
        else tokens.delete(token);
        write(tokens);
        return enabled;
      },
    };
  },
  get id() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "id") || "";
  },
  set id(value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, "id", String(value));
  },
  get value() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "value") || "";
  },
  set value(value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, "value", String(value));
  },
  get open() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "open") !== null;
  },
  set open(value) {
    if (value) __krrNativeDom("setAttribute", this.__krrNodeId, "open", "");
    else __krrNativeDom("removeAttribute", this.__krrNodeId, "open");
  },
  get dataset() {
    const nodeId = this.__krrNodeId;
    return new Proxy(
      {},
      {
        get(_target, property) {
          return (
            __krrNativeDom("getAttribute", nodeId, __krrDatasetAttribute(property)) || undefined
          );
        },
        set(_target, property, value) {
          __krrNativeDom("setAttribute", nodeId, __krrDatasetAttribute(property), String(value));
          return true;
        },
      },
    );
  },
  get style() {
    const nodeId = this.__krrNodeId;
    return new Proxy(
      {},
      {
        get(_target, property) {
          return __krrNativeDom("styleGet", nodeId, String(property)) || "";
        },
        set(_target, property, value) {
          __krrNativeDom("styleSet", nodeId, String(property), String(value));
          return true;
        },
      },
    );
  },
  getAttribute(name) {
    return __krrNativeDom("getAttribute", this.__krrNodeId, String(name));
  },
  setAttribute(name, value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, String(name), String(value));
  },
  removeAttribute(name) {
    __krrNativeDom("removeAttribute", this.__krrNodeId, String(name));
  },
  appendChild(child) {
    __krrNativeDom("appendChild", this.__krrNodeId, child.__krrNodeId);
    return child;
  },
  remove() {
    __krrNativeDom("remove", this.__krrNodeId);
  },
  matches(selector) {
    return __krrNativeDom("closest", this.__krrNodeId, String(selector)) === this.__krrNodeId;
  },
  closest(selector) {
    return __krrElement(__krrNativeDom("closest", this.__krrNodeId, String(selector)));
  },
};
const __krrInlineHandler = (target, event) => {
  const source = target.getAttribute?.(`on${event.type}`);
  if (!source) return;
  const result = Function("event", source).call(target, event);
  if (result === false && event.cancelable) event.preventDefault();
};
const __krrDispatchElementPhase = (target, event, capture, phase) => {
  __krrDispatchTargetPhase(target, event, capture, phase);
  if (!capture && !event.__krrImmediatePropagationStopped) __krrInlineHandler(target, event);
};
const __krrDispatchElementEvent = (target, event) => {
  __krrBeginDispatch(target, event);
  try {
    const nodeIds = __krrNativeDom("eventPath", target.__krrNodeId);
    if (!Array.isArray(nodeIds)) return event;
    const path = nodeIds.map(__krrElement);
    const ancestors = path.slice(1);
    const captures = [window, document, ...ancestors.slice().reverse()];
    for (const currentTarget of captures) {
      __krrDispatchElementPhase(currentTarget, event, true, Event.CAPTURING_PHASE);
      if (event.__krrPropagationStopped) break;
    }
    if (!event.__krrPropagationStopped) {
      __krrDispatchElementPhase(target, event, true, Event.AT_TARGET);
      if (!event.__krrImmediatePropagationStopped)
        __krrDispatchElementPhase(target, event, false, Event.AT_TARGET);
    }
    if (event.bubbles && !event.__krrPropagationStopped) {
      for (const currentTarget of [...ancestors, document, window]) {
        __krrDispatchElementPhase(currentTarget, event, false, Event.BUBBLING_PHASE);
        if (event.__krrPropagationStopped) break;
      }
    }
    return event;
  } finally {
    __krrEndDispatch(event);
  }
};
globalThis.__krrDispatchHostEvent = (nodeId, type, key, bubbles, cancelable) => {
  const event = new Event(type, { bubbles, cancelable });
  if (key !== null && key !== undefined) event.key = String(key);
  return __krrDispatchElementEvent(__krrElement(nodeId), event);
};
let __krrDocumentReadyState = "loading";
globalThis.document = __krrInstallEventTarget({
  getElementById(id) {
    return __krrElement(__krrNativeDom("getElementById", String(id)));
  },
  querySelector(selector) {
    return __krrElement(__krrNativeDom("querySelector", String(selector)));
  },
  querySelectorAll(selector) {
    return __krrNativeDom("querySelectorAll", String(selector)).map(__krrElement);
  },
  createElement(tag) {
    return __krrElement(__krrNativeDom("createElement", String(tag)));
  },
  get body() {
    return __krrElement(__krrNativeDom("querySelector", "body"));
  },
  get readyState() {
    return __krrDocumentReadyState;
  },
});
globalThis.window = globalThis;
__krrInstallEventTarget(globalThis);
globalThis.__krrDispatchDocumentContentLoaded = () => {
  __krrDocumentReadyState = "interactive";
  document.dispatchEvent(new Event("readystatechange"));
  document.dispatchEvent(new Event("DOMContentLoaded"));
};
globalThis.__krrDispatchWindowLoad = () => {
  __krrDocumentReadyState = "complete";
  document.dispatchEvent(new Event("readystatechange"));
  window.dispatchEvent(new Event("load"));
};
