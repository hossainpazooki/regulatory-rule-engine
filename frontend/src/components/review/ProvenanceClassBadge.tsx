/**
 * A visually-distinct badge for one of the four provenance classes. Pure
 * presentational: it renders strictly from `CLASS_STYLE` in `provenance.ts`
 * (color token + icon + accessible label) so the four classes are stable and
 * separable (Playwright baselines depend on this stability — do not randomize).
 *
 * When `isTestKey` is true, it renders a loud "TEST KEY - not production"
 * marker (CLAUDE.md: test keys must be loud, never silent).
 */
import { CLASS_STYLE, type ProvenanceClass } from './provenance'

interface ProvenanceClassBadgeProps {
  klass: ProvenanceClass
  isTestKey?: boolean
}

export function ProvenanceClassBadge({ klass, isTestKey }: ProvenanceClassBadgeProps) {
  const style = CLASS_STYLE[klass]
  const Icon = style.icon

  return (
    <span className="inline-flex items-center gap-2">
      <span
        data-provenance-class={klass}
        role="img"
        aria-label={style.aria}
        title={style.aria}
        className={`inline-flex items-center gap-1.5 border rounded-full px-3 py-1 text-sm font-medium ${style.colorClass}`}
      >
        <Icon aria-hidden="true" className="h-4 w-4" />
        {style.title}
      </span>
      {isTestKey ? (
        <span
          data-testid="test-key-marker"
          className="inline-flex items-center border rounded-full px-2 py-0.5 text-xs font-semibold bg-red-500/20 text-red-300 border-red-500/50"
        >
          TEST KEY - not production
        </span>
      ) : null}
    </span>
  )
}
