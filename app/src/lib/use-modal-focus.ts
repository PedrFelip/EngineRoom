import { type RefObject, useEffect, useRef } from 'react'

/** Keep modal focus and scroll ownership stable when callbacks change. */
export function useModalFocus(
  open: boolean,
  dialogRef: RefObject<HTMLDivElement | null>,
  onEscape: () => void,
) {
  const escapeRef = useRef(onEscape)
  useEffect(() => {
    escapeRef.current = onEscape
  }, [onEscape])

  useEffect(() => {
    const dialog = dialogRef.current
    if (!open || !dialog) return
    const previousFocus = document.activeElement
    const { overflow, paddingRight } = document.body.style
    const scrollbarWidth =
      window.innerWidth - document.documentElement.clientWidth
    document.body.style.overflow = 'hidden'
    if (scrollbarWidth > 0) {
      const padding = Number.parseFloat(
        getComputedStyle(document.body).paddingRight,
      )
      document.body.style.paddingRight = `${padding + scrollbarWidth}px`
    }
    const controls = () =>
      Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button, input, select, textarea, a[href], [tabindex], [contenteditable]',
        ),
      ).filter(
        (element) =>
          element.tabIndex >= 0 &&
          !element.matches(':disabled, [inert], [contenteditable="false"]') &&
          !element.closest('[inert]') &&
          element.getClientRects().length > 0,
      )
    const containFocus = () => {
      const active = document.activeElement
      if (
        !dialog.contains(active) ||
        (active instanceof HTMLElement &&
          (active.matches(':disabled') || active.getClientRects().length === 0))
      ) {
        dialog.focus()
      }
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        escapeRef.current()
      }
      if (event.key !== 'Tab') return
      const elements = controls()
      const index = elements.indexOf(document.activeElement as HTMLElement)
      if (
        index === -1 ||
        (event.shiftKey ? index === 0 : index === elements.length - 1)
      ) {
        event.preventDefault()
        const target =
          (event.shiftKey ? elements[elements.length - 1] : elements[0]) ??
          dialog
        target.focus()
      }
    }
    dialog.focus()
    document.addEventListener('keydown', handleKeyDown)
    document.addEventListener('focusin', containFocus)
    const observer = new MutationObserver(containFocus)
    observer.observe(dialog, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['disabled', 'hidden', 'style', 'class'],
    })
    return () => {
      observer.disconnect()
      document.removeEventListener('keydown', handleKeyDown)
      document.removeEventListener('focusin', containFocus)
      document.body.style.overflow = overflow
      document.body.style.paddingRight = paddingRight
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected) {
        previousFocus.focus()
      }
    }
  }, [open, dialogRef])
}
