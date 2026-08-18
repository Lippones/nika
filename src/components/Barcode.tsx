interface BarcodeProps {
  /** Fonte das barras — normalmente o fingerprint do relay de saída. */
  source: string;
  strong?: boolean;
}

/**
 * Código de barras de verdade: cada dígito hexadecimal do fingerprint define a
 * largura e o tom de uma barra. Fingerprints diferentes desenham códigos
 * diferentes — é dado, não enfeite.
 */
export function Barcode({ source, strong = false }: BarcodeProps) {
  const digits = source.replace(/[^0-9a-f]/gi, "").slice(0, 44).split("");
  if (digits.length === 0) return null;

  return (
    <div
      className={`barcode${strong ? " barcode--strong" : ""}`}
      role="img"
      aria-label={`Fingerprint ${source}`}
    >
      {digits.map((digit, index) => {
        const value = parseInt(digit, 16);
        return (
          <i
            key={`${index}-${digit}`}
            style={{
              width: `${1 + (value % 3)}px`,
              opacity: 0.4 + ((value >> 2) % 4) * 0.2,
            }}
          />
        );
      })}
    </div>
  );
}
