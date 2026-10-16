package main

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"slices"

	"github.com/genshinsim/gcsim/pkg/agg"
	"github.com/genshinsim/gcsim/pkg/gcs/ast"
	"github.com/genshinsim/gcsim/pkg/gcs/eval"
	"github.com/genshinsim/gcsim/pkg/gcs/parser"
	"github.com/genshinsim/gcsim/pkg/shortcut"
	"github.com/genshinsim/gcsim/pkg/simulation"
	"github.com/genshinsim/gcsim/pkg/simulator"
)

type Request struct {
	Action   string `json:"action"` // "validate" | "sample" | "reference"
	Config   string `json:"config,omitempty"`
	Category string `json:"category,omitempty"` // "characters" | "weapons" | "artifacts"
	Seed     uint64 `json:"seed,omitempty"`
}

type Response struct {
	Success bool            `json:"success"`
	Data    json.RawMessage `json:"data,omitempty"`
	Error   string          `json:"error,omitempty"`
}

type ValidateResult struct {
	Valid           bool     `json:"valid"`
	Characters      []string `json:"characters,omitempty"`
	DurationSeconds float64  `json:"duration_seconds,omitempty"`
	TargetCount     int      `json:"target_count,omitempty"`
}

type ReferenceResult struct {
	Category   string   `json:"category"`
	SyntaxRule string   `json:"syntax_rule"`
	Example    string   `json:"example"`
	Keys       []string `json:"keys"`
}

type FailureDetails struct {
	EnergySec  float64 `json:"energy_seconds"`
	StaminaSec float64 `json:"stamina_seconds"`
	SwapSec    float64 `json:"swap_cd_seconds"`
	SkillSec   float64 `json:"skill_cd_seconds"`
	DashSec    float64 `json:"dash_cd_seconds"`
	BurstCDSec float64 `json:"burst_cd_seconds"`
}

func toFailureDetails(f agg.FailureTimes) FailureDetails {
	return FailureDetails{
		EnergySec:  f.Energy,
		StaminaSec: f.Stamina,
		SwapSec:    f.Swap,
		SkillSec:   f.Skill,
		DashSec:    f.Dash,
		BurstCDSec: f.BurstCD,
	}
}

func sumsToMap(s agg.Sums) map[string]float64 {
	m := make(map[string]float64, len(s.Keys))
	for i, k := range s.Keys {
		if i < len(s.Values) {
			m[k] = s.Values[i]
		}
	}
	return m
}

type CharacterSummary struct {
	Name             string             `json:"name"`
	TotalDamage      float64            `json:"total_damage"`
	DamagePercentage float64            `json:"damage_percentage"`
	DPS              float64            `json:"dps"`
	ActiveTimeSec    float64            `json:"active_time_seconds"`
	Actions          map[string]float64 `json:"actions"`
	Reactions        map[string]float64 `json:"reactions"`
	DamageBySource   map[string]float64 `json:"damage_by_source"`
	Failures         FailureDetails     `json:"failures"`
}

type SampleSummary struct {
	Seed            uint64             `json:"seed"`
	DurationSec     float64            `json:"duration_seconds"`
	TotalDamage     float64            `json:"total_damage"`
	DPS             float64            `json:"dps"`
	Characters      []CharacterSummary `json:"characters"`
	OverallFailures FailureDetails     `json:"overall_failures"`
}

func main() {
	var resp Response
	defer func() {
		if r := recover(); r != nil {
			resp = Response{Success: false, Error: fmt.Sprintf("simulation engine panic: %v", r)}
		}
		_ = json.NewEncoder(os.Stdout).Encode(resp)
	}()

	input, err := io.ReadAll(os.Stdin)
	if err != nil {
		resp = Response{Success: false, Error: "failed to read stdin: " + err.Error()}
		return
	}

	var req Request
	if err := json.Unmarshal(input, &req); err != nil {
		resp = Response{Success: false, Error: "invalid request JSON: " + err.Error()}
		return
	}

	switch req.Action {
	case "validate":
		file := ast.NewFile()
		p := parser.New(file, req.Config)
		simcfg, _, err := p.Parse()
		if err != nil {
			resp = Response{Success: false, Error: err.Error()}
			return
		}
		if len(simcfg.Errors) > 0 {
			errMsgs := ""
			for _, v := range simcfg.Errors {
				errMsgs += fmt.Sprintf("%v\n", v)
			}
			resp = Response{Success: false, Error: errMsgs}
			return
		}

		chars := make([]string, len(simcfg.Characters))
		for i, c := range simcfg.Characters {
			chars[i] = c.Base.Key.String()
		}

		valData := ValidateResult{
			Valid:           true,
			Characters:      chars,
			DurationSeconds: simcfg.Settings.Duration,
			TargetCount:     len(simcfg.Targets),
		}
		bytes, _ := json.Marshal(valData)
		resp = Response{Success: true, Data: bytes}

	case "sample":
		seed := req.Seed
		if seed == 0 {
			seed = uint64(simulator.CryptoRandSeed())
		}

		file := ast.NewFile()
		p := parser.New(file, req.Config)
		simcfg, gcsl, err := p.Parse()
		if err != nil {
			resp = Response{Success: false, Error: "parse error: " + err.Error()}
			return
		}
		if len(simcfg.Errors) > 0 {
			errMsgs := ""
			for _, v := range simcfg.Errors {
				errMsgs += fmt.Sprintf("%v\n", v)
			}
			resp = Response{Success: false, Error: errMsgs}
			return
		}

		cpycfg := simcfg.Copy()
		program := gcsl.Copy()

		core, err := simulation.NewCore(int64(seed), false, cpycfg)
		if err != nil {
			resp = Response{Success: false, Error: "core init error: " + err.Error()}
			return
		}

		evaluator, err := eval.NewEvaluator(file, program, core)
		if err != nil {
			resp = Response{Success: false, Error: "evaluator error: " + err.Error()}
			return
		}

		sim, err := simulation.New(cpycfg, evaluator, core)
		if err != nil {
			resp = Response{Success: false, Error: "simulation init error: " + err.Error()}
			return
		}

		result, err := sim.Run()
		if err != nil {
			resp = Response{Success: false, Error: "simulation run error: " + err.Error()}
			return
		}

		summary := agg.Summarize(&result)
		durationSec := float64(summary.Duration) / 60.0

		charSummaries := make([]CharacterSummary, len(summary.Characters))
		for i := range summary.Characters {
			cs := &summary.Characters[i]
			name := ""
			if i < len(simcfg.Characters) {
				name = simcfg.Characters[i].Base.Key.String()
			}

			charDPS := 0.0
			dmgPct := 0.0
			if durationSec > 0 {
				charDPS = cs.Damage / durationSec
			}
			if summary.TotalDamage > 0 {
				dmgPct = (cs.Damage / summary.TotalDamage) * 100.0
			}

			charSummaries[i] = CharacterSummary{
				Name:             name,
				TotalDamage:      cs.Damage,
				DamagePercentage: dmgPct,
				DPS:              charDPS,
				ActiveTimeSec:    float64(cs.ActiveTime) / 60.0,
				Actions:          sumsToMap(cs.Actions),
				Reactions:        sumsToMap(cs.Reactions),
				DamageBySource:   sumsToMap(cs.DamageBySource),
				Failures:         toFailureDetails(cs.Failures),
			}
		}

		sampleData := SampleSummary{
			Seed:            seed,
			DurationSec:     durationSec,
			TotalDamage:     summary.TotalDamage,
			DPS:             summary.DPS,
			Characters:      charSummaries,
			OverallFailures: toFailureDetails(summary.Failures),
		}

		bytes, _ := json.Marshal(sampleData)
		resp = Response{Success: true, Data: bytes}

	case "reference":
		var ref ReferenceResult
		switch req.Category {
		case "characters":
			keys := make([]string, 0, len(shortcut.CharNameToKey))
			for k := range shortcut.CharNameToKey {
				keys = append(keys, k)
			}
			slices.Sort(keys)
			ref = ReferenceResult{
				Category:   "characters",
				SyntaxRule: "<char_name> char lvl=<level>/<asc> cons=<constellation> talent=<n,e,q>;",
				Example:    "raiden char lvl=90/90 cons=0 talent=9,9,10;",
				Keys:       keys,
			}
		case "weapons":
			keys := make([]string, 0, len(shortcut.WeaponNameToKey))
			for k := range shortcut.WeaponNameToKey {
				keys = append(keys, k)
			}
			slices.Sort(keys)
			ref = ReferenceResult{
				Category:   "weapons",
				SyntaxRule: "<char_name> add weapon=\"<weapon_name>\" refine=<1-5> lvl=<level>/<asc>;",
				Example:    "raiden add weapon=\"engulfinglightning\" refine=1 lvl=90/90;",
				Keys:       keys,
			}
		case "artifacts":
			keys := make([]string, 0, len(shortcut.SetNameToKey))
			for k := range shortcut.SetNameToKey {
				keys = append(keys, k)
			}
			slices.Sort(keys)
			ref = ReferenceResult{
				Category:   "artifacts",
				SyntaxRule: "<char_name> add set=\"<set_name>\" count=<2|4>;",
				Example:    "raiden add set=\"emblemofseveredfate\" count=4;",
				Keys:       keys,
			}
		default:
			resp = Response{Success: false, Error: "unknown reference category: " + req.Category + " (supported: characters, weapons, artifacts)"}
			return
		}
		bytes, _ := json.Marshal(ref)
		resp = Response{Success: true, Data: bytes}

	default:
		resp = Response{Success: false, Error: "unknown action: " + req.Action}
	}
}
