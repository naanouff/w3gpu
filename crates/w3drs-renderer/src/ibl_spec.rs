//! Résolutions / échantillonnage du **bake IBL** côté CPU (`IblContext::from_hdr_with_spec`).
//! Le préréglage **max** reprend les constantes historiques (qualité par défaut) ; les autres
//! abaissent surtout la **pré-filtré** (coût dominant) et un peu l’irradiance / la LUT.

/// Paramètres complets d’une génération IBL (cubemaps + LUT) à partir d’un equirect HDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IblGenerationSpec {
    /// Face de la cubemap d’**irradiance** diffuse (Rgba16F).
    pub irradiance_size: u32,
    /// Face de la **pré-filtré** (spec) en mip0 (Rgba16F) — **puissance de 2** recommandée.
    pub prefiltered_size: u32,
    /// Taille 2D de la **LUT** split-sum (Rg16F).
    pub brdf_lut_size: u32,
    /// Échantillons Monte Carlo / texel — irradiance.
    pub irradiance_samples: u32,
    /// Échantillons cible au mip0 — pré-filtré (réduit sur les petits mips).
    pub prefiltered_base_samples: u32,
    /// Taille côté (chaque voie) pour le BRDF intégral (LUT 2D).
    pub brdf_integral_samples: u32,
}

impl IblGenerationSpec {
    /// Qualité maximale (historique) : irradiance 128, pré-filtré 512, LUT 256.
    pub const fn max() -> Self {
        Self {
            irradiance_size: 128,
            prefiltered_size: 512,
            brdf_lut_size: 256,
            irradiance_samples: 1024,
            prefiltered_base_samples: 384,
            brdf_integral_samples: 1024,
        }
    }

    /// Tiers reconnus par nom (champ `ibl_tier` du JSON Phase A). Inconnu → **max** + avertissement.
    pub fn from_tier_name(name: &str) -> Self {
        from_tier_name_str(name, true)
    }

    /// Abaisse les dimensions ; échantillons légèrement réduits.
    pub const fn high() -> Self {
        Self {
            irradiance_size: 64,
            prefiltered_size: 256,
            brdf_lut_size: 256,
            irradiance_samples: 1024,
            prefiltered_base_samples: 384,
            brdf_integral_samples: 1024,
        }
    }

    pub const fn medium() -> Self {
        Self {
            irradiance_size: 32,
            prefiltered_size: 128,
            brdf_lut_size: 128,
            irradiance_samples: 1024,
            prefiltered_base_samples: 256,
            brdf_integral_samples: 512,
        }
    }

    pub const fn low() -> Self {
        Self {
            irradiance_size: 32,
            prefiltered_size: 64,
            brdf_lut_size: 64,
            irradiance_samples: 512,
            prefiltered_base_samples: 256,
            brdf_integral_samples: 512,
        }
    }

    /// Qualité « secours » : bake beaucoup plus court (utile mesures WASM, tests).
    pub const fn min() -> Self {
        Self {
            irradiance_size: 16,
            prefiltered_size: 32,
            brdf_lut_size: 32,
            irradiance_samples: 256,
            prefiltered_base_samples: 128,
            brdf_integral_samples: 256,
        }
    }
}

impl Default for IblGenerationSpec {
    fn default() -> Self {
        Self::max()
    }
}

fn from_tier_name_str(name: &str, log_unknown: bool) -> IblGenerationSpec {
    let n = name.trim().to_ascii_lowercase();
    let s = match n.as_str() {
        "max" | "" | "default" => IblGenerationSpec::max(),
        "high" => IblGenerationSpec::high(),
        "medium" => IblGenerationSpec::medium(),
        "low" => IblGenerationSpec::low(),
        "min" | "minimum" => IblGenerationSpec::min(),
        _other => {
            if log_unknown {
                let raw = name.trim();
                log::warn!("ibl_tier: unknown {raw} — using max", raw = raw);
            }
            IblGenerationSpec::max()
        }
    };
    s
}

/// Même logique que [`IblGenerationSpec::from_tier_name`] sans journaliser l’inconnu (tests).
pub fn from_tier_name_silent(name: &str) -> IblGenerationSpec {
    from_tier_name_str(name, false)
}

/// Nombre de mips de la **pré-filtré** (face mip0 = `mip0_size` puissance de 2).
#[inline]
pub fn prefiltered_mip_level_count(mip0_size: u32) -> u32 {
    mip0_size.trailing_zeros() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_max_is_largest_prefilter() {
        let m = IblGenerationSpec::max();
        let lo = IblGenerationSpec::min();
        assert!(m.prefiltered_size > lo.prefiltered_size);
        assert_eq!(m, IblGenerationSpec::from_tier_name("max"));
        assert_eq!(lo, IblGenerationSpec::from_tier_name("min"));
    }

    #[test]
    fn from_tier_silent_unknown() {
        let d = from_tier_name_silent("nope");
        assert_eq!(d, IblGenerationSpec::max());
    }
}
