import Image, { type ImageOptions } from '@tiptap/extension-image'
import { Plugin, PluginKey } from '@tiptap/pm/state'

export type ImageLayout = 'inline' | 'wrap-left' | 'wrap-right' | 'float'

export const ANTE_IMAGE_DRAG_EVENT = 'ante-image-drag'

const MAX_IMAGE_BYTES = 10 * 1024 * 1024
export const MIN_IMAGE_WIDTH = 48

function readFileAsDataURL(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = reject
    reader.readAsDataURL(file)
  })
}

export const AnteImage = Image.extend({
  draggable: false,
  selectable: true,
  addOptions(): ImageOptions {
    const parent = this.parent?.() as ImageOptions | undefined
    return {
      ...(parent ?? ({} as ImageOptions)),
      allowBase64: true,
    }
  },
  addAttributes() {
    return {
      ...this.parent?.(),
      layout: {
        default: 'inline' as ImageLayout,
        parseHTML: (el) => (el.getAttribute('data-layout') as ImageLayout) ?? 'inline',
        renderHTML: (attrs) => ({ 'data-layout': attrs.layout }),
      },
      floatX: {
        default: 20,
        parseHTML: (el) => parseFloat(el.getAttribute('data-float-x') ?? '20'),
        renderHTML: (attrs) =>
          attrs.layout === 'float' ? { 'data-float-x': String(attrs.floatX) } : {},
      },
      floatY: {
        default: 20,
        parseHTML: (el) => parseFloat(el.getAttribute('data-float-y') ?? '20'),
        renderHTML: (attrs) =>
          attrs.layout === 'float' ? { 'data-float-y': String(attrs.floatY) } : {},
      },
      caption: {
        default: '',
        parseHTML: (el) => el.getAttribute('data-caption') ?? '',
        renderHTML: (attrs) =>
          attrs.caption ? { 'data-caption': attrs.caption } : {},
      },
      width: {
        default: null as number | null,
        parseHTML: (el) => {
          const w = el.getAttribute('width') ?? el.getAttribute('data-width')
          if (!w) return null
          const n = parseInt(w, 10)
          return Number.isFinite(n) && n > 0 ? n : null
        },
        renderHTML: (attrs) =>
          attrs.width ? { width: String(attrs.width) } : {},
      },
    }
  },

  addNodeView() {
    return ({ node: initNode, getPos, editor }) => {
      let currentLayout = initNode.attrs.layout as ImageLayout
      let currentFloatX = initNode.attrs.floatX as number
      let currentFloatY = initNode.attrs.floatY as number
      let currentCaption = (initNode.attrs.caption as string) ?? ''
      let currentWidth = (initNode.attrs.width as number | null) ?? null
      let currentTextAlign = (initNode.attrs.textAlign as string | null) ?? 'left'
      let isDragging = false
      let captionHasFocus = false
      let activeDragCleanup: (() => void) | null = null

      const wrapper = document.createElement('div')
      wrapper.className = 'ante-image-wrapper'
      wrapper.setAttribute('contenteditable', 'false')

      const frame = document.createElement('span')
      frame.className = 'ante-image-frame'

      const img = document.createElement('img')
      img.src = initNode.attrs.src as string
      img.draggable = false
      if (initNode.attrs.alt) img.alt = initNode.attrs.alt as string
      if (initNode.attrs.title) img.title = initNode.attrs.title as string

      type HandleDir = 'nw' | 'ne' | 'sw' | 'se'
      const HANDLE_LABELS: Record<HandleDir, string> = {
        nw: 'Resize from top-left (NW)',
        ne: 'Resize from top-right (NE)',
        sw: 'Resize from bottom-left (SW)',
        se: 'Resize from bottom-right (SE)',
      }
      const handles: Record<HandleDir, HTMLSpanElement> = {} as Record<
        HandleDir,
        HTMLSpanElement
      >
      ;(['nw', 'ne', 'sw', 'se'] as const).forEach((dir) => {
        const h = document.createElement('span')
        h.className = `ante-image-resize-handle ante-image-resize-${dir}`
        h.setAttribute('data-handle', dir)
        h.setAttribute('title', HANDLE_LABELS[dir])
        h.setAttribute('aria-label', HANDLE_LABELS[dir])
        h.setAttribute('role', 'button')
        handles[dir] = h
      })

      frame.appendChild(img)
      frame.appendChild(handles.nw)
      frame.appendChild(handles.ne)
      frame.appendChild(handles.sw)
      frame.appendChild(handles.se)

      const caption = document.createElement('span')
      caption.className = 'ante-image-caption'
      caption.setAttribute('contenteditable', 'true')
      caption.setAttribute('role', 'textbox')
      caption.setAttribute('aria-label', 'Image caption')
      caption.setAttribute('data-placeholder', 'Add caption')
      caption.textContent = currentCaption

      wrapper.appendChild(frame)
      wrapper.appendChild(caption)

      const commitCaption = () => {
        const text = (caption.textContent ?? '').replace(/\u00a0/g, ' ').trim()
        if (text === currentCaption) return
        currentCaption = text
        const pos = typeof getPos === 'function' ? getPos() : undefined
        if (pos === undefined) return
        const tr = editor.view.state.tr.setNodeAttribute(pos, 'caption', text)
        editor.view.dispatch(tr)
      }

      caption.addEventListener('mousedown', (e) => e.stopPropagation())
      caption.addEventListener('focus', () => {
        captionHasFocus = true
      })
      caption.addEventListener('blur', () => {
        captionHasFocus = false
        if (caption.innerHTML === '<br>') caption.textContent = ''
        commitCaption()
      })
      caption.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          caption.blur()
        } else if (e.key === 'Escape') {
          e.preventDefault()
          caption.textContent = currentCaption
          caption.blur()
        }
      })

      const applyWidth = () => {
        if (currentWidth) {
          img.style.width = `${currentWidth}px`
          img.style.height = 'auto'
          img.style.maxWidth = 'none'
        }
      }

      const applyLayout = (
        layout: ImageLayout,
        floatX: number,
        floatY: number,
        textAlign: string,
      ) => {
        currentLayout = layout
        currentFloatX = floatX
        currentFloatY = floatY
        currentTextAlign = textAlign
        img.setAttribute('data-layout', layout)

        wrapper.style.cssText = ''
        img.style.cssText = ''
        wrapper.removeAttribute('data-float-wrapper')

        if (layout === 'wrap-left') {
          wrapper.style.cssText =
            'float:left;margin:4px 16px 8px 0;display:inline-block;' +
            (currentWidth ? '' : 'max-width:50%;')
          img.style.cssText = 'max-width:100%;height:auto;display:block;'
        } else if (layout === 'wrap-right') {
          wrapper.style.cssText =
            'float:right;margin:4px 0 8px 16px;display:inline-block;' +
            (currentWidth ? '' : 'max-width:50%;')
          img.style.cssText = 'max-width:100%;height:auto;display:block;'
        } else if (layout === 'float') {
          wrapper.style.cssText =
            `position:absolute;left:${floatX}px;top:${floatY}px;z-index:2;cursor:move;display:inline-block;user-select:none;` +
            (currentWidth ? '' : 'max-width:300px;')
          img.style.cssText =
            'display:block;max-width:100%;height:auto;pointer-events:none;user-select:none;'
          wrapper.setAttribute('data-float-wrapper', 'true')
        } else {
          // inline (block-level): alignment via margin on a fit-content wrapper
          let marginLeft = '0'
          let marginRight = 'auto'
          if (textAlign === 'center') {
            marginLeft = 'auto'
            marginRight = 'auto'
          } else if (textAlign === 'right') {
            marginLeft = 'auto'
            marginRight = '0'
          } else if (textAlign === 'justify') {
            marginLeft = '0'
            marginRight = 'auto'
          }
          wrapper.style.cssText = `display:block;width:fit-content;max-width:100%;margin-left:${marginLeft};margin-right:${marginRight};`
          img.style.cssText = 'max-width:100%;height:auto;display:block;'
        }

        applyWidth()
      }

      const dispatchDrag = (phase: 'start' | 'end') => {
        editor.view.dom.dispatchEvent(
          new CustomEvent(ANTE_IMAGE_DRAG_EVENT, { detail: { phase } }),
        )
      }

      // Float image move drag
      wrapper.addEventListener('mousedown', (e: MouseEvent) => {
        if (currentLayout !== 'float' || e.button !== 0) return
        const target = e.target as HTMLElement
        if (target.closest('.ante-image-caption')) return
        if (target.closest('.ante-image-resize-handle')) return
        const startX = e.clientX
        const startY = e.clientY
        let dx = 0
        let dy = 0
        let moved = false

        const onMove = (e: MouseEvent) => {
          dx = e.clientX - startX
          dy = e.clientY - startY
          if (!moved && (Math.abs(dx) > 2 || Math.abs(dy) > 2)) {
            moved = true
            isDragging = true
            wrapper.style.willChange = 'transform'
            wrapper.style.transition = 'none'
            wrapper.style.opacity = '0.85'
            dispatchDrag('start')
          }
          if (moved) {
            wrapper.style.transform = `translate3d(${dx}px, ${dy}px, 0)`
          }
        }

        const onUp = () => {
          window.removeEventListener('mousemove', onMove)
          window.removeEventListener('mouseup', onUp)
          activeDragCleanup = null
          if (!moved) {
            isDragging = false
            return
          }
          const parent = wrapper.offsetParent as HTMLElement | null
          const parentRect = parent?.getBoundingClientRect()
          const wrapperRect = wrapper.getBoundingClientRect()
          let finalX = currentFloatX
          let finalY = currentFloatY
          if (parentRect) {
            finalX = Math.max(0, wrapperRect.left - parentRect.left)
            finalY = Math.max(0, wrapperRect.top - parentRect.top)
          }
          wrapper.style.willChange = ''
          wrapper.style.opacity = ''
          wrapper.style.transform = ''
          isDragging = false
          dispatchDrag('end')
          const pos = typeof getPos === 'function' ? getPos() : undefined
          if (pos !== undefined) {
            editor
              .chain()
              .setNodeSelection(pos)
              .updateAttributes('image', { floatX: finalX, floatY: finalY })
              .run()
          }
        }

        window.addEventListener('mousemove', onMove)
        window.addEventListener('mouseup', onUp)
        activeDragCleanup = () => {
          window.removeEventListener('mousemove', onMove)
          window.removeEventListener('mouseup', onUp)
        }
      })

      // Force NodeSelection on click so the handle reliably appears even if
      // ProseMirror's default atom-node click handling misses (e.g. when the
      // click lands on our inline caption instead of the img).
      wrapper.addEventListener('mousedown', (e: MouseEvent) => {
        const target = e.target as HTMLElement
        if (target.closest('.ante-image-caption')) return
        if (target.closest('.ante-image-resize-handle')) return
        const pos = typeof getPos === 'function' ? getPos() : undefined
        if (pos === undefined) return
        console.debug('[AnteImage] select', { pos })
        editor.chain().setNodeSelection(pos).run()
      })

      const startResize = (e: MouseEvent, dir: HandleDir) => {
        if (e.button !== 0) return
        e.preventDefault()
        e.stopPropagation()
        console.debug('[AnteImage] resize:start', { dir, client: [e.clientX, e.clientY] })

        const growsWithDx = dir === 'ne' || dir === 'se'
        const startX = e.clientX
        const startWidth = img.offsetWidth || img.naturalWidth || 100
        const startHeight = img.offsetHeight || img.naturalHeight || 100
        const aspect = startHeight > 0 ? startWidth / startHeight : 1
        let moved = false

        const onMove = (ev: MouseEvent) => {
          const rawDx = ev.clientX - startX
          const dx = growsWithDx ? rawDx : -rawDx
          if (!moved && Math.abs(rawDx) > 2) {
            moved = true
            isDragging = true
            dispatchDrag('start')
          }
          if (!moved) return
          const w = Math.max(MIN_IMAGE_WIDTH, Math.round(startWidth + dx))
          img.style.width = `${w}px`
          img.style.height = `${Math.round(w / aspect)}px`
          img.style.maxWidth = 'none'
          wrapper.style.maxWidth = 'none'
        }

        const onUp = () => {
          window.removeEventListener('mousemove', onMove)
          window.removeEventListener('mouseup', onUp)
          activeDragCleanup = null
          if (!moved) {
            isDragging = false
            console.debug('[AnteImage] resize:cancel (no-move)', { dir })
            return
          }
          const newWidth = img.offsetWidth
          currentWidth = newWidth
          img.style.height = 'auto'
          isDragging = false
          dispatchDrag('end')
          console.debug('[AnteImage] resize:end', { dir, newWidth })
          const pos = typeof getPos === 'function' ? getPos() : undefined
          if (pos !== undefined) {
            editor
              .chain()
              .setNodeSelection(pos)
              .updateAttributes('image', { width: newWidth })
              .run()
          }
        }

        window.addEventListener('mousemove', onMove)
        window.addEventListener('mouseup', onUp)
        activeDragCleanup = () => {
          window.removeEventListener('mousemove', onMove)
          window.removeEventListener('mouseup', onUp)
        }
      }

      ;(['nw', 'ne', 'sw', 'se'] as const).forEach((dir) => {
        handles[dir].addEventListener('mousedown', (e) => startResize(e, dir))
      })

      applyLayout(
        initNode.attrs.layout as ImageLayout,
        initNode.attrs.floatX as number,
        initNode.attrs.floatY as number,
        ((initNode.attrs.textAlign as string | null) ?? 'left'),
      )

      return {
        dom: wrapper,
        update(newNode) {
          if (newNode.type.name !== 'image') return false
          if (img.src !== (newNode.attrs.src as string)) {
            img.src = newNode.attrs.src as string
          }
          img.alt = (newNode.attrs.alt as string | null) ?? ''
          img.title = (newNode.attrs.title as string | null) ?? ''
          const incomingCaption = (newNode.attrs.caption as string) ?? ''
          if (!captionHasFocus && incomingCaption !== currentCaption) {
            currentCaption = incomingCaption
            caption.textContent = incomingCaption
          }
          const incomingWidth = (newNode.attrs.width as number | null) ?? null
          if (incomingWidth !== currentWidth) {
            currentWidth = incomingWidth
          }
          applyLayout(
            newNode.attrs.layout as ImageLayout,
            newNode.attrs.floatX as number,
            newNode.attrs.floatY as number,
            ((newNode.attrs.textAlign as string | null) ?? 'left'),
          )
          return true
        },
        selectNode() {
          wrapper.classList.add('ante-image-selected')
        },
        deselectNode() {
          wrapper.classList.remove('ante-image-selected')
        },
        destroy() {
          activeDragCleanup?.()
          activeDragCleanup = null
        },
        stopEvent(event) {
          if (
            event.target instanceof HTMLElement &&
            (event.target.closest('.ante-image-caption') ||
              event.target.closest('.ante-image-resize-handle'))
          ) {
            return true
          }
          return isDragging
        },
        ignoreMutation(mutation) {
          if (
            mutation.target instanceof HTMLElement &&
            mutation.target.closest('.ante-image-caption')
          ) {
            return true
          }
          if (
            mutation.target instanceof Text &&
            mutation.target.parentElement?.closest('.ante-image-caption')
          ) {
            return true
          }
          return false
        },
      }
    }
  },

  addProseMirrorPlugins() {
    const editor = this.editor
    return [
      new Plugin({
        key: new PluginKey('anteImagePasteDrop'),
        props: {
          handleDOMEvents: {
            paste(_view, event) {
              const items = Array.from(
                (event as ClipboardEvent).clipboardData?.items ?? [],
              )
              const imageItems = items.filter((item) => item.type.startsWith('image/'))
              if (!imageItems.length) return false
              event.preventDefault()
              for (const item of imageItems) {
                const file = item.getAsFile()
                if (!file) continue
                if (file.size > MAX_IMAGE_BYTES) {
                  console.warn(
                    `[ante] Pasted image exceeds ${MAX_IMAGE_BYTES} bytes and was skipped`,
                    file.name,
                    file.size,
                  )
                  continue
                }
                const name = file.name || 'pasted-image'
                readFileAsDataURL(file).then((src) => {
                  editor.chain().focus().setImage({ src, title: name, alt: name }).run()
                })
              }
              return true
            },
            drop(_view, event) {
              const files = Array.from(
                (event as DragEvent).dataTransfer?.files ?? [],
              )
              const imageFiles = files.filter((f) => f.type.startsWith('image/'))
              if (!imageFiles.length) return false
              event.preventDefault()
              for (const file of imageFiles) {
                if (file.size > MAX_IMAGE_BYTES) {
                  console.warn(
                    `[ante] Dropped image exceeds ${MAX_IMAGE_BYTES} bytes and was skipped`,
                    file.name,
                    file.size,
                  )
                  continue
                }
                const name = file.name || 'dropped-image'
                readFileAsDataURL(file).then((src) => {
                  editor.chain().focus().setImage({ src, title: name, alt: name }).run()
                })
              }
              return true
            },
          },
        },
      }),
    ]
  },
})
