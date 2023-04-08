// https://gist.github.com/yy0931/9e5fe637b861e1c669be24fdb3f6da58

class ScrollbarEvent extends Event {
    constructor(type: string, public delta: number) { super(type) }
}

interface ScrollbarEventMap {
    "change": ScrollbarEvent
}

export interface Scrollbar extends HTMLElement {
    addEventListener<K extends keyof ScrollbarEventMap>(type: K, listener: (this: Scrollbar, ev: ScrollbarEventMap[K]) => any, options?: boolean | AddEventListenerOptions): void;
    addEventListener(type: string, listener: EventListenerOrEventListenerObject, options?: boolean | AddEventListenerOptions): void;
    removeEventListener<K extends keyof ScrollbarEventMap>(type: K, listener: (this: Scrollbar, ev: ScrollbarEventMap[K]) => any, options?: boolean | EventListenerOptions): void;
    removeEventListener(type: string, listener: EventListenerOrEventListenerObject, options?: boolean | EventListenerOptions): void;
}

export const scrollbarWidth = "13px"

export abstract class Scrollbar extends HTMLElement {
    readonly #root
    readonly #handle
    readonly #background
    #min = 0
    #max = 100  // #min <= #size
    #size = 30  // 0 < #size <= #max - #min
    #value = 0  // #min <= #value <= #max - #size

    static get observedAttributes() { return ["min", "max", "size", "value"] as const }
    #horizontal

    #width() { return this.#horizontal ? "height" : "width" }
    #height() { return this.#horizontal ? "width" : "height" }
    #vh() { return this.#horizontal ? "vw" : "vh" }
    #top() { return this.#horizontal ? "left" : "top" }
    #Y() { return this.#horizontal ? "X" : "Y" }

    constructor(horizontal: boolean) {
        super()
        this.#horizontal = horizontal
        this.#root = this.attachShadow({ mode: "open" })
        this.#root.innerHTML = `\
<style>
:host {
    display: block;
    ${this.#height()}: 20${this.#vh()};
    line-height: 0;
}
.handle {
    position: relative;
    background: rgb(131, 131, 131);
    ${this.#width()}: ${scrollbarWidth};
    top: 0;
    z-index: 1;
}
.handle:hover {
    background: rgb(111, 111, 111);
}
.handle:active {
    background: rgb(90, 90, 90);
}
.background {
    ${this.#height()}: 100%;
    display: inline-block;
    background: #e1e1e1;
}
</style>
<div class="background" id="background"><div class="handle" id="handle"></div></div>
`
        this.#handle = this.#root.querySelector<HTMLDivElement>("#handle")!
        this.#background = this.#root.querySelector<HTMLDivElement>("#background")!

        const startDragging = (e: MouseEvent) => {
            const pageY = e[`page${this.#Y()}`]
            const value = this.#value

            const onDrag = (e: MouseEvent) => {
                e.stopImmediatePropagation()
                const oldValue = this.#value
                this.value = value + (e[`page${this.#Y()}`] - pageY) / this.#background.getBoundingClientRect()[this.#height()] * (this.#max - this.#min)
                if (Math.round(this.#value) !== Math.round(oldValue)) {
                    this.dispatchEvent(new ScrollbarEvent("change", this.#value - oldValue))
                }
            }

            window.addEventListener("mousemove", onDrag)
            window.addEventListener("mouseup", () => {
                window.removeEventListener("mousemove", onDrag)
            }, { once: true })
        }
        this.#handle.addEventListener("mousedown", (e) => {
            e.preventDefault()
            e.stopImmediatePropagation()
            startDragging(e)
        })

        this.#background.addEventListener("mousedown", (e) => {
            e.preventDefault()
            const background = this.#background.getBoundingClientRect()
            const oldValue = this.#value
            this.value = (e[`client${this.#Y()}`] - background[this.#top()]) / background[this.#height()] * (this.#max - this.#min) + this.#min - this.#size / 2
            if (Math.round(this.#value) !== Math.round(oldValue)) {
                this.dispatchEvent(new ScrollbarEvent("change", this.#value - oldValue))
            }
            startDragging(e)
        })

        this.#render()
    }

    attributeChangedCallback(name: typeof Scrollbar["observedAttributes"][number], oldValue: string | null, newValue: string | null) {
        if (newValue === null) { return }
        if (name === "min") { this.min = +newValue; return }
        if (name === "max") { this.max = +newValue; return }
        if (name === "size") { this.size = +newValue; return }
        if (name === "value") { this.value = +newValue; return }
    }

    get min() { return this.#min }
    get max() { return this.#max }
    get size() { return this.#size }
    get value() { return this.#value }

    wheel(delta: number) {
        const oldValue = this.#value
        this.value += delta
        this.dispatchEvent(new ScrollbarEvent("change", this.#value - oldValue))
    }

    #rescaled() {
        this.#value = Math.max(this.#min, Math.min(this.#max - this.#size, this.#value))
        this.#render()
    }
    set min(v: number) { v = +v; if (!Number.isFinite(v)) { return } this.#min = v; this.#rescaled() }
    set max(v: number) { v = +v; if (!Number.isFinite(v)) { return } this.#max = v; this.#rescaled() }
    set size(v: number) { v = +v; if (!Number.isFinite(v)) { return } this.#size = v; this.#rescaled() }
    set value(v: number) {
        v = +v
        if (!Number.isFinite(v)) { return }
        this.#value = Math.max(this.#min, Math.min(this.#max - this.#size, v))
        this.#render()
    }

    #render() {
        if (this.#max <= this.#min || this.#max - this.#min <= this.#size) {
            this.#handle.style[this.#height()] = "100%"
            this.#handle.style[this.#top()] = "0%"
        } else {
            const handleHeight = Math.max(this.#size / (this.#max - this.#min) * 100, 10)
            const top = (this.#value - this.#min) / (this.#max - this.#min - this.#size) * 100 * (1 - handleHeight / 100)
            this.#handle.style[this.#height()] = Math.max(0, Math.min(100, handleHeight)) + "%"
            this.#handle.style[this.#top()] = Number.isFinite(top) ? Math.max(0, Math.min(100, top)) + "%" : "0%"
        }
    }
}

export class ScrollbarX extends Scrollbar { constructor() { super(true) } }
export class ScrollbarY extends Scrollbar { constructor() { super(false) } }

customElements.define("scrollbar-x", ScrollbarX)
customElements.define("scrollbar-y", ScrollbarY)
