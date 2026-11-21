local diff = {
	["axisDiffs"] = {
		["a2001cdnil"] = {
			["added"] = {
				[1] = {
					["filter"] = {
						["curvature"] = {
							[1] = 0.1,
						},
						["deadzone"] = 0.02,
						["invert"] = true,
						["saturationX"] = 1,
						["saturationY"] = 1,
						["slider"] = false,
					},
					["key"] = "JOY_Y",
				},
			},
			["name"] = "Pitch",
		},
	},
	["keyDiffs"] = {
		["d3001pnilu3001cd12vd1vpnilvu0"] = {
			["name"] = "Weapon release",
			["removed"] = {
				[1] = {
					["key"] = "JOY_BTN1",
				},
			},
		},
		["d68pnilunilcdnilvdnilvpnilvunil"] = {
			["added"] = {
				[1] = {
					["key"] = "JOY_BTN5",
				},
			},
			["name"] = "Landing gear up/down",
		},
	},
}
return diff
