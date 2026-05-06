package com.enel.virtualentity.batch.equipment.extractor.controller;

import com.enel.virtualentity.batch.equipment.agui.conf.CountryRoutingDataSource;
import com.enel.virtualentity.batch.equipment.agui.entity.AguiColumnsRecordDTO;
import com.enel.virtualentity.batch.equipment.agui.entity.RecordDTO;
import com.enel.virtualentity.batch.equipment.agui.mapper.AguiRecordColumnsMapper;
import com.enel.virtualentity.batch.equipment.agui.mapper.LogComponAlertMapper;
import com.enel.virtualentity.batch.equipment.common.ExtractorConstants;
import com.enel.virtualentity.batch.equipment.extractor.controller.model.JobExecutionInfo;
import com.enel.virtualentity.batch.equipment.extractor.dto.*;
import com.enel.virtualentity.batch.equipment.extractor.entity.*;
import com.enel.virtualentity.batch.equipment.extractor.entity.lock.CategoriesRunning;
import com.enel.virtualentity.batch.equipment.extractor.exception.ErrorResponseTO;
import com.enel.virtualentity.batch.equipment.extractor.exception.TooManyRequestException;
import com.enel.virtualentity.batch.equipment.extractor.repository.ParametersRepository;
import com.enel.virtualentity.batch.equipment.extractor.service.old.ExtractorService;
import com.enel.virtualentity.batch.equipment.transformer.JsonForDataLoader;
import com.enel.virtualentity.batch.equipment.transformer.JsonForDeletions;
import com.enel.virtualentity.batch.equipment.transformer.JsonForMongo;
import com.enel.virtualentity.dto.LogDTO;
import com.enel.virtualentity.dto.elasticsearch.ResponseValue;
import com.enel.virtualentity.enums.AINTypesEnum;
import com.enel.virtualentity.enums.LogMessageTypeEnum;
import com.enel.virtualentity.enums.LogStatusTypeEnum;
import com.enel.virtualentity.exception.ValueNotAllowedException;
import com.enel.virtualentity.service.*;
import com.enel.virtualentity.service.ioda.IodaDocTools;
import com.enel.virtualentity.service.topology.TopologyTableService;
import com.enel.virtualentity.utils.mapper.MapperCache;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.SerializationFeature;
import com.google.gson.*;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.collections4.CollectionUtils;
import org.apache.commons.collections4.MapUtils;
import org.apache.commons.lang3.StringUtils;
import org.apache.poi.ss.usermodel.Row;
import org.apache.poi.ss.usermodel.Sheet;
import org.apache.poi.ss.usermodel.Workbook;
import org.apache.poi.ss.usermodel.WorkbookFactory;
import org.joda.time.DateTime;
import org.springframework.batch.core.*;
import org.springframework.batch.core.launch.*;
import org.springframework.batch.core.repository.JobExecutionAlreadyRunningException;
import org.springframework.batch.core.repository.JobInstanceAlreadyCompleteException;
import org.springframework.batch.core.repository.JobRestartException;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Profile;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.transaction.annotation.Transactional;
import org.springframework.web.bind.annotation.*;

import javax.sql.DataSource;
import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.text.ParseException;
import java.text.SimpleDateFormat;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.time.format.DateTimeParseException;
import java.util.*;
import java.util.stream.Collectors;
import org.modelmapper.ModelMapper;
import org.springframework.web.multipart.MultipartFile;

import static com.enel.virtualentity.batch.equipment.transformer.JsonForDataLoader.*;
import static com.enel.virtualentity.service.AINServiceImpl.*;
import static com.enel.virtualentity.service.RulesServices.PAR_EXT_SYS_SUBEQU;
import static com.enel.virtualentity.service.datatransformers.Constants.*;
import static org.springframework.http.MediaType.APPLICATION_JSON_VALUE;

@RestController
@RequestMapping("/extractor")
@Profile("read-write")
@Slf4j
public class ExtractorControllerImpl implements ExtractorController{

    private static final String CATEGORIES_TO_RUN_FOR = "CATEGORIES_TO_RUN_FOR_";
    private static final String CATEGORIES_TO_CHECK_FOR = "CATEGORIES_TO_CHECK_FOR_";

    private final String msgError ="There are $ request in pending";

    private final int MILLI_TO_HOUR = 1000 * 60 * 60;

    @Autowired
    @Qualifier("multiRoutingDataSource")
    protected DataSource multiRoutingDataSource;

    @Autowired
    private MONGOService mongoService;

    @Autowired
    JsonForDataLoader jsonForDataLoader;

    @Autowired
    JsonForMongo jsonForMongo;

    @Autowired
    JsonForDeletions jsonForDeletions;

    @Autowired
    private JobLauncher extractorJobLauncher;

    //@Autowired
    //private JobExplorer extractorJobExplorer;

    @Autowired
    private Job jobAgui;

    @Autowired
    @Qualifier("jobConductorType")
    private Job jobConductorType;

    @Autowired
    @Qualifier("jobConductorTypeDBRules")
    private Job jobConductorTypeDBRules;

    @Autowired
    @Qualifier("jobTransformerType")
    private Job jobTransformerType;

    @Autowired
    @Qualifier("jobTransformerTypeDBRules")
    private Job jobTransformerTypeDBRules;

    @Autowired
    @Qualifier("jobGrDeviceType")
    private Job jobGrDeviceType;

    @Autowired
    @Qualifier("jobGrDeviceTypeDBRules")
    private Job jobGrDeviceTypeDBRules;

    @Autowired
    @Qualifier("jobAuiDocument")
    private Job jobAuiDocument;

    @Autowired
    private ExtractorService extractorService;

    @Autowired
    private ParametersRepository parametersRepository;

    @Autowired
    private LogService logService;

    @Autowired
    private OrquestrationLogService orquestrationLogService;

    @Autowired
    LoaderService loaderService;

    @Autowired
    AguiService aguiService;

    @Autowired
    IodaDocTools auiService;

    private ModelMapper modelMapper = new ModelMapper();

    private ObjectMapper mapper = new ObjectMapper();

    @Autowired
    private RulesServices rulesServices;

    @Autowired
    private LockDbService lockDbService;

    @Autowired
    private JobService jobService;

    @Autowired
    private OrchestrationService orchestrationService;

    @Autowired
    private TopologyTableService topologyTableService;

    @Autowired
    private LogComponAlertTableService logComponAlertTableService;

    @Autowired
    private DataLakeService dataLakeService;

    @Autowired
    AsyncLauncherForOrquestrationStarter asyncLauncherForOrquestrationStarter;

    @Value("${edp.extractor.max-concurrent-job:3}")
    private int defaultCountInProgress;
    private Set<String> categoriesWithAD;

    private Map<String ,Integer> maxCategoriesToRunPerCountry = new HashMap<>();
    private Map<String ,Integer> maxCategoriesToCheckPerCountry = new HashMap<>();

    /* UNCOMMENT TO WRITE LOG TRACES ON orquestration_log table

    private Integer idLogFortraces;

    DateTimeFormatter formatter = DateTimeFormatter.ofPattern("dd-MM-yyyy HH:mm:ss.SSS");

     */

    @Override
    @GetMapping(value = "/agui/extractAguiToAIN", produces = "application/json")
    public ResponseEntity<String> startBatchAguiToAIN(String country, String aguiCategory, String ainCategory, String type) {
        log.info("Begin startBatchAgui");

        Long jobId = 0L;
        Rules rule;


        Parameters p = parametersRepository.findByCountryAndEnvironmentAndName(COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase(), "MAX_CHUNK");

        if (p != null){

            List<CategoriesMapping> categories = rulesServices.getExtraccion(country, aguiCategory, ainCategory, type);

            if (!categories.isEmpty() && categories.size() == 1) {

                CategoriesMapping categoryMapping = categories.get(0);

                if (orchestrationService.checkConditiosToStartExtraction(categories.get(0).getRules().iterator().next(), true, true)){

                    rule = categoryMapping.getRules().iterator().next();
                    if (rule.isEnableCreation())
                        jobId = extractAgui(country, aguiCategory, ainCategory, type, null, null, null, Integer.parseInt(p.getValue()), -1, -1, false, AUTO_MODE, null, null);
                    else
                        jobId = extractAgui(country, aguiCategory, ainCategory, type, null, null, null, Integer.parseInt(p.getValue()), -1, -1, false, AUTO_UPDATE_MODE, null, null);
                }else{
                    return new ResponseEntity<>("{ \"status\": \"KO\", \"error\": \"Another job is currently working for this rule or the rule is not in Ready status\"}", HttpStatus.BAD_REQUEST);
                }
            }else{
                if (categories.isEmpty()){
                    return new ResponseEntity<>("{ \"status\": \"KO\", \"error\": \"No rule found for input data\"}", HttpStatus.BAD_REQUEST);
                }else
                    return new ResponseEntity<>("{ \"status\": \"KO\", \"error\": \"More than 1 rule found for input data\"}", HttpStatus.BAD_REQUEST);
            }

        }else{
            return new ResponseEntity<>("{\"status\": \"KO\", \"error\": \"MAX_CHUNK parameter not found in the rules db\"}", HttpStatus.BAD_REQUEST);
        }

        log.info("End startBatchAgui");


        // Update data in the rule
       orchestrationService.updateRuleData(rule, jobId, type);

        return new ResponseEntity<>(String.format("{ \"status\": \"ok\", \"job_execution_id\" : %.0f }", jobId.floatValue()), HttpStatus.OK);

    }



    @Override
    @GetMapping(value = "/agui/rulesInProgress", produces = "application/json")
    public ResponseEntity<String> rulesInProgress(String country){

        JsonObject results;
        JsonObject result;
        JsonArray rules;
        List<Object> rulesInProgres;
        Object[] row;
        int count;

        results = new JsonObject();
        rules = new JsonArray();

        rulesInProgres = rulesServices.getRulesInProgressByCountry(country);

        count =rulesInProgres.size();


        for(Object o : rulesInProgres){
            result = new JsonObject();
            row = (Object[])o;

            result.addProperty("aguiCategory", (String)row[2]);
            result.addProperty("ainCategory", (String)row[1]);
            result.addProperty("entityType", (String)row[0]);
            rules.add(result);
        }

        results.addProperty("count", count);
        results.add("categories", rules);

        return new ResponseEntity<>(results.toString(), HttpStatus.OK);

    }

    @Override
    @GetMapping(value = "/agui/autoCheckStatus", produces = "application/json")
    public ResponseEntity<String> autoCheckStatus(String country){

        List<CategoriesRunning> cr;
        JsonArray results = new JsonArray();
        JsonObject result;
        List<Log> requestToCheck;
        List<String> statuses = new ArrayList<>();
        String realStatus;
        String logStatus;
        Optional<Rules> rule;
        Integer retries;
        int maxRetries = 3;
        int maxHours;

        if (StringUtils.isNotBlank(country))
            log.error("[ORCHESTRATOR] Begin AutoCheckStatus for: " + country);
        else
            log.error("[ORCHESTRATOR] Begin AutoCheckStatus for: ALL");

        Map<String, String> parameters = rulesServices.getParameters(ComparationService.COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase());

        OrquestrationLog logTransformator = orquestrationLogService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
        if (StringUtils.isBlank(country)) {
            logTransformator.setSessionName("AUTOCHECKSTATUS for ALL countries");
            logTransformator.setCountry("ALL");
        }else{
            logTransformator.setSessionName("AUTOCHECKSTATUS for " + country);
            logTransformator.setCountry(country);
        }
        logTransformator.setCreationDate(new Date());
        orquestrationLogService.updateLog(logTransformator);


        maxRetries = Integer.parseInt(parameters.getOrDefault("MAX_RETRIES_FOR_DATALOADER", "3"));
        maxHours = Integer.parseInt(parameters.getOrDefault("HOURS_TO_DISCARD_REQUEST", "6"));

        statuses.add("READY");
        statuses.add("DONE");
        statuses.add("DISCARDED");
        statuses.add("PENDING");

        requestToCheck = logService.getAllRequestToRecheckStatus(statuses, country);
        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Number of request to check: " + requestToCheck);

        for (Log l : requestToCheck){

            if (StringUtils.isBlank(l.getRequestId()) || (StringUtils.isNotBlank(l.getMigrationStatus()) && l.getMigrationStatus().equalsIgnoreCase("DISCARDED")))
                continue;

            logStatus = l.getMigrationStatus();

            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Working on request_id: " + l.getRequestId() + " Real status: " + logStatus);

            if (l.getIdRules() != null) {

                rule = rulesServices.getRuleById(l.getIdRules());

                if (rule.isPresent() && (l.getRequestId().startsWith("Error:")  ||
                        (logStatus != null && (logStatus.equalsIgnoreCase("FAILED") || logStatus.equalsIgnoreCase("ERROR"))))) {

                    autoCheckStatusTreatmentForError(rule.get(), l, logTransformator, maxRetries, results, logStatus, logStatus);

                } else {

                    if (rule.isPresent())
                        autoCheckStatusTreatmentForUnknownStatus(rule.get(), l, logTransformator, maxRetries, maxHours, results, logStatus);

                }
            }

        }

        logTransformator.setJsonResult(results.toString());
        logTransformator.setEndDate(new Date());
        orquestrationLogService.updateLog(logTransformator);

        if (StringUtils.isNotBlank(country))
            log.error("[ORCHESTRATOR] End AutoCheckStatus for: " + country);
        else
            log.error("[ORCHESTRATOR] End AutoCheckStatus for: ALL");


        return new ResponseEntity<>(results.toString(), HttpStatus.OK);

    }

    @Transactional
    protected void autoCheckStatusTreatmentForUnknownStatus(Rules rule, Log l, OrquestrationLog logTransformator, int maxRetries, int maxHours, JsonArray results, String realStatus){

        JsonObject result = new JsonObject();
        long hours = -1;
        List<CategoriesRunning> cr;

        if (l.getCreationDate() != null)
            hours = (new Date().getTime() - (l.getCreationDate().getTime())) / MILLI_TO_HOUR;

        result.addProperty("request_id", l.getRequestId());
        result.addProperty("migration_status", l.getMigrationStatus());
        result.addProperty("rules_id", l.getIdRules());
        result.addProperty("rules_status", rule.getCurrentExecutionStatus());

        if (hours >= maxHours) {

            if (l.getCountRetry() != null && l.getCountRetry() == maxRetries) {

                rule.setCurrentJobExecutionId(null);
                rule.setCurrentExecutionStatus("ERROR-AMS");
                rulesServices.updateRules(rule.getId(), rule);

                // Discard AD/HC in PENDING (to avoid that the rule stay IN_PROGRESS because there are request different from DONE or DISCARDED)
                if (l.getJobExecutionId() != -1)
                    logService.updateMigrationStatusForPendingRequestWithJobId("DISCARDED", l.getJobExecutionId());
                else
                    logService.updateMigrationStatusForPendingRequestWithoutJobId("DISCARDED", l.getCountry(), l.getAguiCategory(), l.getAinCategory(), l.getEntityType());

                CategoriesMapping catMapAux = rule.getCategoriesMapping();

                //orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, getIdLogFortraces(), LocalDateTime.now().format(formatter) +
                //       " autoCheckStatusTreatmentForUnknownStatus: " + catMapAux.getCountry() + "_" + catMapAux.getAguiCategory() +
                //       catMapAux.getAinCategory() + "_" + rule.getEntityType() + " Status updated to: " + rule.getCurrentExecutionStatus() + " due to inactivitiy of the request: " + l.getRequestId());

                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.ERROR, logTransformator.getId(), "ISSUE: Rule for request_id: " + l.getRequestId() + " =>  " + rule.getCategoriesMapping().getAguiCategory() + UNDERSCORE_SEPARATOR + rule.getCategoriesMapping().getAinCategory() + " setted to ERROR-AMS (maximum time for request in error exceeded and request have reached the maximum retries)");

                // Delete the record in the Semaphore
                cr = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow(rule.getCategoriesMapping().getCountry(),
                        rule.getCategoriesMapping().getAguiCategory(), rule.getCategoriesMapping().getAinCategory(), "AGUI-TO-AIN");
                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule for request_id: " + l.getRequestId() + " =>  " + rule.getCategoriesMapping().getAguiCategory() + UNDERSCORE_SEPARATOR + rule.getCategoriesMapping().getAinCategory() + " Records in semaphore: " + cr.size());

                if (!cr.isEmpty() && cr.size() == 1) {
                    lockDbService.deleteRecordRunningCategory(cr.get(0).getId());
                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule for request_id: " + l.getRequestId() + " =>  " + rule.getCategoriesMapping().getAguiCategory() + UNDERSCORE_SEPARATOR + rule.getCategoriesMapping().getAinCategory() + " Record in semaphore deleted.");
                }

                l.setMigrationStatus("DISCARDED");
                logService.updateLog(new ObjectMapper().convertValue(l, LogDTO.class));

            }else{
                l.setMigrationStatus(realStatus);
                logService.updateLog(new ObjectMapper().convertValue(l, LogDTO.class));
                JsonObject resultRetry = checkRequestRetries(rule, maxRetries, new ObjectMapper().convertValue(l, LogDTO.class));
                rulesServices.updateRules(rule.getId(), rule);

                CategoriesMapping catMapAux = rule.getCategoriesMapping();

                //orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, getIdLogFortraces(), LocalDateTime.now().format(formatter) +
                //      " autoCheckStatusTreatmentForUnknownStatus: " + catMapAux.getCountry() + "_" + catMapAux.getAguiCategory() +
                //      catMapAux.getAinCategory() + "_" + rule.getEntityType() + " After call to checkRequestRetries, Status updated to: " + rule.getCurrentExecutionStatus());

                if (resultRetry != null && resultRetry.has("status") && resultRetry.get("status").getAsString().equalsIgnoreCase("OK")) {
                    result.addProperty("new_request_id", resultRetry.get("new_request_id").getAsString());
                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "request_id: " + l.getRequestId() + " retried");
                }
            }


        }
        result.addProperty("new_migration_status", l.getMigrationStatus());
        result.addProperty("new_rules_status", rule.getCurrentExecutionStatus());

        results.add(result);


    }

    @Transactional
    protected void autoCheckStatusTreatmentForError(Rules rule, Log l, OrquestrationLog logTransformator, int maxRetries, JsonArray results, String logStatus, String realStatus){

        JsonObject result = new JsonObject();
        List<CategoriesRunning> cr;

        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule for request_id: " + l.getRequestId() + " =>  " + rule.getCategoriesMapping().getAguiCategory() + UNDERSCORE_SEPARATOR + rule.getCategoriesMapping().getAinCategory() + " log status: " + logStatus);

        result.addProperty("request_id", l.getRequestId());
        result.addProperty("migration_status", l.getMigrationStatus());
        result.addProperty("rules_id", l.getIdRules());
        result.addProperty("rules_status", rule.getCurrentExecutionStatus());
        result.addProperty("rules_retries", rule.getCountRetry());

        if (rule.getCurrentExecutionStatus().equalsIgnoreCase("ERROR-AMS")) {
            l.setMigrationStatus("DISCARDED");
            logService.updateLog(new ObjectMapper().convertValue(l, LogDTO.class));
            result.addProperty("new_migration_status", l.getMigrationStatus());
            results.add(result);
            // Delete the record in the Semaphore
            cr = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow(rule.getCategoriesMapping().getCountry(),
                    rule.getCategoriesMapping().getAguiCategory(), rule.getCategoriesMapping().getAinCategory(), "AGUI-TO-AIN");

            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.ERROR, logTransformator.getId(), "ISSUE: request_id: " + l.getRequestId() + " setted to disscarded (rule is in ERROR-AMS status)");
            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.ERROR, logTransformator.getId(), "ISSUE: Rule for request_id: " + l.getRequestId() + " =>  " + rule.getCategoriesMapping().getAguiCategory() + UNDERSCORE_SEPARATOR + rule.getCategoriesMapping().getAinCategory() + " Records in semaphore: " + cr.size());
            if (!cr.isEmpty() && cr.size() == 1) {
                lockDbService.deleteRecordRunningCategory(cr.get(0).getId());
                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule for request_id: " + l.getRequestId() + " =>  " + rule.getCategoriesMapping().getAguiCategory() + UNDERSCORE_SEPARATOR + rule.getCategoriesMapping().getAinCategory() + " Record in semaphore deleted.");
            }

        } else {

            if (l.getCountRetry() != null && l.getCountRetry() == maxRetries) {
                l.setMigrationStatus("DISCARDED");
                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.ERROR, logTransformator.getId(), "ISSUE: request_id: " + l.getRequestId() + " setted to DISCARDED (we have reached the maximum retries)");
                logService.updateLog(new ObjectMapper().convertValue(l, LogDTO.class));
                rule.setCurrentJobExecutionId(null);
                rule.setCurrentExecutionStatus("ERROR-AMS");
                rulesServices.updateRules(rule.getId(), rule);

                // Discard AD/HC in PENDING (to avoid that the rule stay IN_PROGRESS because there are request different from DONE or DISCARDED)
                if (l.getJobExecutionId() != -1)
                    logService.updateMigrationStatusForPendingRequestWithJobId("DISCARDED", l.getJobExecutionId());
                else
                    logService.updateMigrationStatusForPendingRequestWithoutJobId("DISCARDED", l.getCountry(), l.getAguiCategory(), l.getAinCategory(), l.getEntityType());

                CategoriesMapping catMapAux = rule.getCategoriesMapping();

                //orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, getIdLogFortraces(), LocalDateTime.now().format(formatter) +
                //      " autoCheckStatusTreatmentForError: " + catMapAux.getCountry() + "_" + catMapAux.getAguiCategory() +
                //      catMapAux.getAinCategory() + "_" + rule.getEntityType() + " Status updated to: " + rule.getCurrentExecutionStatus() + " due to max retriers of the request: " + l.getRequestId());

                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.ERROR, logTransformator.getId(), "ISSUE: Rule for request_id: " + l.getRequestId() + " =>  " + rule.getCategoriesMapping().getAguiCategory() + UNDERSCORE_SEPARATOR + rule.getCategoriesMapping().getAinCategory() + " setted to ERROR-AMS (the request have reached the maximum retries)");
                // Delete the record in the Semaphore
                cr = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow(rule.getCategoriesMapping().getCountry(),
                        rule.getCategoriesMapping().getAguiCategory(), rule.getCategoriesMapping().getAinCategory(), "AGUI-TO-AIN");

                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule for request_id: " + l.getRequestId() + " =>  " + rule.getCategoriesMapping().getAguiCategory() + UNDERSCORE_SEPARATOR + rule.getCategoriesMapping().getAinCategory() + " Records in semaphore: " + cr.size());
                if (!cr.isEmpty() && cr.size() == 1) {
                    lockDbService.deleteRecordRunningCategory(cr.get(0).getId());
                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule for request_id: " + l.getRequestId() + " =>  " + rule.getCategoriesMapping().getAguiCategory() + UNDERSCORE_SEPARATOR + rule.getCategoriesMapping().getAinCategory() + " Record in semaphore deleted.");
                }

            } else {
                l.setMigrationStatus(realStatus);
                logService.updateLog(new ObjectMapper().convertValue(l, LogDTO.class));
                JsonObject resultRetry = checkRequestRetries(rule, maxRetries, new ObjectMapper().convertValue(l, LogDTO.class));
                rulesServices.updateRules(rule.getId(), rule);

                CategoriesMapping catMapAux = rule.getCategoriesMapping();

                //orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, getIdLogFortraces(), LocalDateTime.now().format(formatter) +
                //      " autoCheckStatusTreatmentForError: " + catMapAux.getCountry() + "_" + catMapAux.getAguiCategory() +
                //      catMapAux.getAinCategory() + "_" + rule.getEntityType() + " After checkRequestRetries, Status updated to: " + rule.getCurrentExecutionStatus() + " due to max retriers of the request: " + l.getRequestId());

                if (resultRetry != null && resultRetry.has("status") && resultRetry.get("status").getAsString().equalsIgnoreCase("OK")) {
                    result.addProperty("new_request_id", resultRetry.get("new_request_id").getAsString());
                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "request_id: " + l.getRequestId() + " retried");
                }

            }

            result.addProperty("new_migration_status", l.getMigrationStatus());
            result.addProperty("new_rules_status", rule.getCurrentExecutionStatus());
            result.addProperty("new_rules_retries", rule.getCountRetry());
            results.add(result);

        }

    }

    @Override
    @GetMapping(value = "/agui/loopAutoRequestsToEnqueue", produces = "application/json")
    public ResponseEntity<String> loopAutoRequestsToEnqueue(String country) throws ParseException {

        ResponseEntity<String> result;

        do{

            result = autoRequestsToEnqueue(null);
            System.out.println("CVR: " + result.getBody());


        }while(result.getBody() != null && !result.getBody().equals("{\"message\":\"No request found in status READY\"}"));

        return new ResponseEntity<>("No quedan más request...", HttpStatus.OK);

    }


    @Override
    @GetMapping(value = "/agui/autoRequestsToEnqueue", produces = "application/json")
    public ResponseEntity<String> autoRequestsToEnqueue(String country) throws ParseException {

        JsonArray results = new JsonArray();
        List<LogDTO> requestsToProcess = new ArrayList<>();
        List<LogDTO> requests;
        List<LogDTO> requestsLca = new ArrayList<>();
        JsonObject result;
        int numberOfRequest;
        Map<String, Set<String>> categoriesSelected = new HashMap<>();
        String category;
        Set<String> countries;
        List<String> statuses = new ArrayList<>();

        String previousExecution = checkIfAutoRequestToEnqueueIsRunningWithSemaphore();

        if (StringUtils.isNotBlank(previousExecution)) {

            OrquestrationLog logTransformator = orquestrationLogService.addNewLog(LogStatusTypeEnum.PROCESSING, null);

            result = new JsonObject();
            result.addProperty("message", previousExecution);
            results.add(result);
            if (StringUtils.isBlank(country)) {
                logTransformator.setSessionName("AUTOREQUESTTOENQUEUE for ALL countries");
                logTransformator.setCountry("ALL");
            } else {
                logTransformator.setSessionName("AUTOREQUESTTOENQUEUE for " + country);
                logTransformator.setCountry(country);
            }
            logTransformator.setEntityType("ALL");
            logTransformator.setCreationDate(new Date());
            logTransformator.setEndDate(new Date());
            logTransformator.setJsonResult(results.toString());

            orquestrationLogService.updateLog(logTransformator);
            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), previousExecution);

            return new ResponseEntity<>(results.toString(), HttpStatus.OK);

        }else{

            if (!blockAutoRequesToEnqueueOnSemaphore()) {
                return new ResponseEntity<>("Could not block the operation in the semaphore", HttpStatus.INTERNAL_SERVER_ERROR);
            }

            if (StringUtils.isNotBlank(country))
                log.error("[ORCHESTRATOR] Begin AutoRequestToEnqueue for: " + country);
            else
                log.error("[ORCHESTRATOR] Begin AutoRequestToEnqueue for: ALL");

            statuses.add("READY");
            statuses.add("NEW");
            statuses.add("RETRY");

            OrquestrationLog logTransformator = orquestrationLogService.addNewLog(LogStatusTypeEnum.PROCESSING, null);

            if (StringUtils.isBlank(country)) {
                logTransformator.setSessionName("AUTOREQUESTTOENQUEUE for ALL countries");
                logTransformator.setCountry("ALL");
            } else {
                logTransformator.setSessionName("AUTOREQUESTTOENQUEUE for " + country);
                logTransformator.setCountry(country);
            }

            logTransformator.setCreationDate(new Date());
            orquestrationLogService.updateLog(logTransformator);

            Map<String, String> parameters = rulesServices.getParameters(ComparationService.COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase());

            numberOfRequest = Integer.parseInt(parameters.getOrDefault("REQUEST_TO_ENQUEUE", "10"));

            if (StringUtils.isBlank(country))
                requests = logService.getLogByStatusAndModeAndLimit(statuses, AUTO_MODE, numberOfRequest);
            else
                requests = logService.getLogByStatusAndCountryAndModeAndLimit(statuses, country, AUTO_MODE, numberOfRequest);

            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Number of request ready to be enqueue: " + requests.size());

            if (!requests.isEmpty()) {

                for (LogDTO l : requests) {

                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Category: " + l.getCountry() + "_" + l.getAguiCategory() + l.getAinCategory() + " request_id: " + l.getRequestId() + " selected to be enqueued");

                    category = l.getAguiCategory() + l.getAinCategory();
                    if (categoriesSelected.containsKey(category)) {
                        categoriesSelected.get(category).add(l.getCountry());
                    } else {
                        countries = new HashSet<>();
                        countries.add(l.getCountry());
                        categoriesSelected.put(category, countries);
                    }

                    requestsToProcess.add(l);

                    if (requestsToProcess.size() == numberOfRequest)
                        break;

                }

            }

            // If we don't have reached the maximun number of request, lets try to get HierarchyCahange and AguiDeletion requests too
            if (requestsToProcess.size() < numberOfRequest)
                requestsLca = getLocComponAlertRequestReady(country, numberOfRequest);

            if (!requestsLca.isEmpty()) {

                for (LogDTO l : requestsLca) {

                    category = l.getAguiCategory() + l.getAinCategory();
                    if (!(categoriesSelected.containsKey(category) && categoriesSelected.get(category).contains(l.getCountry()))) {
                        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Category: " + l.getCountry() + "_" + category + " request_id: " + l.getRequestId() + " selected to be enqueued (is a log_compon_alert request)");
                        requestsToProcess.add(l);
                        if (requestsToProcess.size() == numberOfRequest)
                            break;
                    }

                }

            }


            if (requests.isEmpty() && requestsLca.isEmpty()) {

                result = new JsonObject();
                result.addProperty("message", "No request found in status READY");
                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "No request found in status ready");
                logTransformator.setJsonResult(results.toString());
                logTransformator.setEndDate(new Date());
                orquestrationLogService.updateLog(logTransformator);
                // Unblock entityType on semaphore
                List<CategoriesRunning> cr = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow("ALL", "AE", "AE", "ORCHESTRATOR");
                if (cr.size() == 1)
                    lockDbService.deleteRecordRunningCategory(cr.getFirst().getId());
                return new ResponseEntity<>(result.toString(), HttpStatus.NO_CONTENT);

            }

            // For Mongo, we are going to process synchronously the requests obtained

            for (LogDTO r : requestsToProcess) {

                mongoService.manageRequestToWriteInMongo(r, results, logTransformator.getId(), true);

            }

            logTransformator.setJsonResult(results.toString());
            logTransformator.setEndDate(new Date());
            orquestrationLogService.updateLog(logTransformator);


            if (StringUtils.isNotBlank(country))
                log.error("[ORCHESTRATOR] End AutoRequestToEnqueue for: " + country);
            else
                log.error("[ORCHESTRATOR] End AutoRequestToEnqueue for: ALL");

            // Unblock entityType on semaphore
            List<CategoriesRunning> cr = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow("ALL", "AE", "AE", "ORCHESTRATOR");
            if (cr.size() == 1)
                lockDbService.deleteRecordRunningCategory(cr.getFirst().getId());

            return new ResponseEntity<>(results.toString(), HttpStatus.OK);
        }

    }



    @Override
    @GetMapping(value = "/agui/processMongoRequestForJobId", produces = "application/json")
    public ResponseEntity<String> processMongoRequestsForJobId(int jobId, boolean force, boolean onlyErrors, boolean processOnlyReady){

        JsonArray results = new JsonArray();
        List<LogDTO> requests;
        JsonObject result;

        log.error("Procesing Mongo requests for jobId: " + jobId);

        requests = logService.getLogsByJob(jobId, true, processOnlyReady);

        if (requests.isEmpty() ){

            result = new JsonObject();
            result.addProperty("message", "No requests found for jobId: " + jobId);
            return new ResponseEntity<>(result.toString(), HttpStatus.OK);

        }else {

            mongoService.processMongoRequest(requests, results, force, onlyErrors);
        }

        return new ResponseEntity<>(results.toString(), HttpStatus.OK);

    }

    @Override
    @GetMapping(value = "/agui/processMongoRequest", produces = "application/json")
    public ResponseEntity<String> processMongoRequest(String requestId, Integer logId, boolean force){

        JsonArray results = new JsonArray();
        Optional<LogDTO> request;
        JsonObject result;

        log.error("Procesing Mongo request: " + requestId + " log Id: " + logId);

        try {
            //request = logService.getLogByRequestId(requestId);
            if (logId != null)
                request = logService.getLogByRequestIdAndId(requestId, logId);
            else
                request = logService.getLogByRequestId(requestId);

            if (request.isEmpty()){

                result = new JsonObject();
                result.addProperty("message", "RequestId: " + requestId + " not found!!");
                return new ResponseEntity<>(result.toString(), HttpStatus.OK);

            }else{
                try {
                    mongoService.processMongoRequest(request.stream().toList(), results, force, false);
                }catch(Exception e){
                    return new ResponseEntity<>("Error processing requestId => " + e.getMessage(), HttpStatus.INTERNAL_SERVER_ERROR);
                }
            }

            return new ResponseEntity<>(results.toString(), HttpStatus.OK);
        }catch (Exception e){
            return new ResponseEntity<>("Error retrieving requestId. Check that the requestId is unique. If not, please, provide also the logId", HttpStatus.BAD_REQUEST);
        }



    }



    private  List<LogDTO> getLocComponAlertRequestReady(String country, int limit){

        List<LogDTO> requests;
        List<String> modes = new ArrayList<>();
        List<String> statuses = new ArrayList<>();

        modes.add(AUTO_HIERARCHY_CHANGE_MODE);
        modes.add(AUTO_AGUI_DELETE_MODE);

        statuses.add("READY");
        statuses.add("NEW");
        statuses.add("RETRY");

        if (StringUtils.isBlank(country))
            requests = logService.getLogByStatusAndModes(statuses, modes, limit);
        else
            requests = logService.getLogByStatusAndCountryAndModes(statuses, country, modes, limit);


        return requests;

    }

    @Override
    @PatchMapping (value = "/agui/forceRequestStatus", consumes =  APPLICATION_JSON_VALUE, produces = APPLICATION_JSON_VALUE)
    @Transactional
    public synchronized ResponseEntity<String> forceRequestStatus(@RequestBody ForceDataLoaderRequestDTO dataLoaderData){

        JsonObject result;
        Optional<LogDTO> logRecord;
        HttpStatus responseStatus = HttpStatus.OK;
        String retryResult;
        String originalStatus;
        String requestId;
        String status;
        ResponseEntity<String> tpResult;

        if (dataLoaderData != null && StringUtils.isNotBlank(dataLoaderData.getRequest_id()))
            log.error("[CALLBACK] Begin foreceRequestStatus for: " + dataLoaderData.getRequest_id());
        else
            log.error("[CALLBACK] Begin foreceRequestStatus without input information needed");

        requestId = dataLoaderData.getRequest_id();
        status = dataLoaderData.getStatus();

        OrquestrationLog logTransformator = orquestrationLogService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
        logTransformator.setSessionName("FORCEREQUESTSTATUS for " + requestId + " status: " + status);
        logTransformator.setCreationDate(new Date());
        orquestrationLogService.updateLog(logTransformator);


        result = new JsonObject();

        logRecord = logService.getLogByRequestId(requestId);

        if (logRecord.isPresent()) {

            originalStatus = logRecord.get().getMigrationStatus();
            logTransformator.setCountry(logRecord.get().getCountry());

            if (status.equalsIgnoreCase("PENDING")){
                if (!logRecord.get().getMigrationStatus().equalsIgnoreCase("NEW")){
                    logRecord.get().setMigrationStatus("NEW");
                    logService.updateLog(logRecord.get());
                }
                result.addProperty("message", "Request id: " + requestId + " Original status: " + originalStatus + " updated to NEW (still is PENDING in data loader)");
                logTransformator.setJsonResult(result.toString());
            }else{
                if (status.equalsIgnoreCase("PARTIAL") || status.equalsIgnoreCase("FAILED"))
                    status = "ERROR";

                logRecord.get().setMigrationStatus(status);
                logService.updateLog(logRecord.get());
                result.addProperty("message", "Request id: " + requestId + " Original status: " + originalStatus + " updated to: " + status);

                // If is a request_id for Equipments, and the status in DataLoader is some ERROR, then we need to retry because
                // we have missed the callback so we have not updated the topology_table. IF the status is DONE, we
                // receive here the equipments to update in topology
                if ((logRecord.get().getEntityType().equalsIgnoreCase(AINTypesEnum.EQUIPMENT.getCode()) || logRecord.get().getEntityType().equalsIgnoreCase(AINTypesEnum.SUBEQ.getCode())) &&
                    !status.equalsIgnoreCase("NEW")) {

                    if (!status.equalsIgnoreCase("DONE")) {
                        retryResult = processRetry(logRecord.get(), "", true);
                        result.add("result", JsonParser.parseString(retryResult).getAsJsonObject());
                    }else{

                        tpResult = updateTopologyTable(logRecord.get(), dataLoaderData);
                        result.addProperty("updateTopologyTableResult", tpResult.getBody());

                    }
                }
                logTransformator.setJsonResult(result.toString());
            }

            if (status.equalsIgnoreCase("DONE")){
                if (logRecord.get().getOperationMode().startsWith("AUTO")) {
                    Rules rule = logService.getLogRules(logRecord.get());
                    ResponseEntity<String> ruleDoneResult = orchestrationService.checkIfRuleIsDone(logRecord.get(), rule);
                    result.addProperty("isRuleDoneResult", ruleDoneResult.getBody());
                    logTransformator.setJsonResult(result.toString());
                }
            }


        }else {


            result.addProperty("message", "Request id: " + requestId + " not found");
            logTransformator.setJsonResult(result.toString());
            responseStatus = HttpStatus.PRECONDITION_FAILED;
        }

        logTransformator.setEndDate(new Date());
        orquestrationLogService.updateLog(logTransformator);

        if (dataLoaderData != null && StringUtils.isNotBlank(dataLoaderData.getRequest_id()))
            log.error("[CALLBACK] End foreceRequestStatus for: " + dataLoaderData.getRequest_id());
        else
            log.error("[CALLBACK] End foreceRequestStatus wihout input information needed");

        return new ResponseEntity<>(result.toString(), responseStatus);

    }

    private ResponseEntity<String> updateTopologyTable(LogDTO log, ForceDataLoaderRequestDTO dataLoaderData){

        JsonObject message = null;
        ResponseEntity<String> result;

        switch(log.getOperationMode()){

            case FULL_MIGRATION_MODE:
            case AUTO_MODE:
            case CREATION_MODE:
                return topologyTableService.callBackForFullMigration(dataLoaderData.getStatus(), dataLoaderData.getEquipments(), log);


            case ONLY_XML_MODE:
            case UPDATE_MODE:
            case AUTO_UPDATE_MODE:

                dataLoaderData.setEquipments(new ArrayList<>());
                return topologyTableService.callBackForFullMigration(dataLoaderData.getStatus(), dataLoaderData.getEquipments(), log);

            case HIERARCHY_CHANGE_MODE:
            case AUTO_HIERARCHY_CHANGE_MODE:

                ResponseEntity<String> resultLCA;
                resultLCA = topologyTableService.callBackToUpdateStatusInLogComponAlert(dataLoaderData.getStatus(), log);
                return resultLCA;

            case AGUI_DELETE_MODE:
            case AUTO_AGUI_DELETE_MODE:

                result = topologyTableService.callBackToUpdateStatusInLogComponAlert(dataLoaderData.getStatus(), log);
                if (result.getStatusCode() != HttpStatus.OK) {
                    JsonObject errorMessage = JsonParser.parseString(result.getBody()).getAsJsonObject();
                    message = new JsonObject();
                    message.addProperty("message1", errorMessage.get("message").getAsString());
                }

                result = topologyTableService.callBackToDeleteTopologyTable(dataLoaderData.getStatus(), dataLoaderData.getEquipments(), null, log);

                if (message == null )
                    return result;
                else {
                    JsonObject errorMessage = JsonParser.parseString(result.getBody()).getAsJsonObject();
                    message.add("message2", errorMessage);
                    return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                            .body(message.toString());
                }

            default:
                return ResponseEntity.status(HttpStatus.NOT_FOUND)
                        .body("{\"message\": \"Unknow mode: " + log.getOperationMode() +  "\"}");

        }

    }

    @Override
    @GetMapping(value = "/agui/autoRetries", produces = "application/json")
    public ResponseEntity<String> autoRetries(@RequestParam(required = false) String country){

        JsonArray results = new JsonArray();
        List<LogDTO> requests;
        JsonObject result;
        String retryResult;
        List<Integer> jobIdsDiscarded = new ArrayList<>();
        Optional<Log> toBeDiscarded;
        ObjectMapper mapper = new ObjectMapper();

        requests = logService.getLogToBeRetried(country);

        if (!requests.isEmpty()){

            for (LogDTO l : requests) {

                // If the job_execution_id is currently not discarded
                if (!jobIdsDiscarded.contains(l.getJobExecutionId())) {

                    // Check if this request have been retried at least 3 times
                    toBeDiscarded = logService.haveBeeingRetry3Times(l.getRequestId());

                    // If so, we discard all the request for this job
                    if (toBeDiscarded.isPresent()) {
                        discardAllRequestOfJob(toBeDiscarded.get().getJobExecutionId(), toBeDiscarded.get().getIdRules());
                        jobIdsDiscarded.add(toBeDiscarded.get().getJobExecutionId());
                        result = new JsonObject();
                        result.addProperty("request_id", l.getRequestId());
                        result.addProperty("job_id", l.getJobExecutionId());
                        result.addProperty("action", "All request updated to DISCARDED. This request have failed at least 3 times");
                        results.add(result);
                    }//If not, we retry the request
                    else {
                        retryResult = processRetry(l, "", true);
                        result = JsonParser.parseString(retryResult).getAsJsonObject();
                        if (result.has("status") && result.get("status").getAsString().equalsIgnoreCase("ko"))
                            result.addProperty("request_id", l.getRequestId());
                        results.add(result);

                    }

                }
            }

        }else{
            result = new JsonObject();
            result.addProperty("message", "No request_id found to be retried");
            results.add(result);

        }

        return new ResponseEntity<>(results.toString(), HttpStatus.OK);
    }

    @Override
    @GetMapping(value = "/agui/starter", produces = "application/json")
    public ResponseEntity<String> extractionStarter(String country, String entityType) throws ParseException {

        asyncLauncherForOrquestrationStarter.launchStarterOperation(country, entityType, orquestrationLogService, parametersRepository, rulesServices,
                logComponAlertTableService, orchestrationService, dataLakeService, lockDbService, aguiService, mongoService, extractorJobLauncher, jobAgui,
                logService, multiRoutingDataSource);

        return new ResponseEntity<>("Operation launched in background. Check the orquestration logs...", HttpStatus.OK);

        /*

        JsonArray results = new JsonArray();
        Map<String, JsonObject> resultMap =new HashMap<>();
        JsonObject result;
        List<Rules> rulesReady;
        List<Object> rulesReadRs;
        List<String> aguiCategoriesTreated = new ArrayList<>();
        Optional<Rules> rule;
        CategoriesMapping categoryToBeExecuted;
        Map<String, List<String >> allRules;
        String date;
        Date dateFrom;
        int count;
        int categoriesChecked = 0;
        int maxCategoriesToRun;
        int maxCategoriesToCheck;
        long jobId;
        SimpleDateFormat sdf = new SimpleDateFormat("dd-MM-yyyy HH:mm:ss");
        SimpleDateFormat sdf2 = new SimpleDateFormat("dd-MM-yyyy HH:mm:ss");
        List<CategoriesMapping> categoriesToBeExecuted = new ArrayList<>();
        JsonObject hcResult = new JsonObject();
        JsonObject adResult = new JsonObject();
        Map<String, Map<String, String>> parametersByCountry = new HashMap<>();
        Map<String, String> parameters;
        OrquestrationLog logTransformator;
        String previousExecution;
        Long countErrorAms;

        if (StringUtils.isBlank(country))
            country = "ALL";

        log.error("[ORCHESTRATOR] Begin Stater for: " + country + "_" + entityType);

        Parameters p = parametersRepository.findByCountryAndEnvironmentAndName(COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase(), "DEFAULT_CHUNK");
        Parameters pMaxHoursInProgress = parametersRepository.findByCountryAndEnvironmentAndName(
                COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase(), "MAX_HOURS_IN_PROGRESS");

        previousExecution = checkIfPreviousEntityIsRunningWithSemaphore(entityType, pMaxHoursInProgress);

        //restoreAbbandonedInProgressRules(entityType, pMaxHoursInProgress);

        if (StringUtils.isNotBlank(previousExecution)){

            logTransformator = orquestrationLogService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
            logTransformator.setSessionName("LOCAL STARTER for " + country + UNDERSCORE_SEPARATOR + entityType);
            logTransformator.setCountry(country);
            logTransformator.setEntityType(entityType);
            logTransformator.setCreationDate(new Date());
            logTransformator.setEndDate(new Date());

            result = new JsonObject();
            result.addProperty("message", previousExecution);
            results.add(result);
            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), previousExecution);

            logTransformator.setJsonResult(results.toString());
            orquestrationLogService.updateLog(logTransformator);

            return new ResponseEntity<>(results.toString(), HttpStatus.OK);

        }

        mapper.disable(SerializationFeature.FAIL_ON_EMPTY_BEANS);

        Set<String> categoriesWithHC = new HashSet<>();
        Set<String> categoriesWithAD = new HashSet<>();
        Map<String, List<Integer>> idsForHCAD = new HashMap<>();
        Map<String ,Integer> minutesPerCountry = new HashMap<>();
        AINTypesEnum ainType;

        logTransformator = orquestrationLogService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
        logTransformator.setSessionName("LOCAL STARTER for " + country + UNDERSCORE_SEPARATOR + entityType);
        logTransformator.setCountry(country);
        logTransformator.setEntityType(entityType);
        logTransformator.setCreationDate(new Date());

        if (!blockEntityTypeOnSemaphore(entityType, logTransformator)){
            return new ResponseEntity<>("Could not block the entityType: " + entityType + " in the semaphore", HttpStatus.OK);
        }

        int maxRetries;
        int minutes;

        ainType = AINTypesEnum.valueOf(entityType);

        maxRetries = Integer.parseInt(parametersRepository.findByCountryAndEnvironmentAndName(COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase(), "MAX_RETRIES_FOR_DATALOADER").getValue());

        rulesReadRs = rulesServices.getRulesReadyToBeExecutedForType(entityType, country);
        rulesReady = convertObjectsToRules(rulesReadRs);

        rulesReady.sort(Comparator.comparing(Rules::getLastExecutionDate));
        allRules = getRulesMap(rulesReady, entityType);

        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rules ready to be executed: " + rulesReady.size());

        result = new JsonObject();
        if (!rulesReady.isEmpty()) {

            for (Rules tempRule : rulesReady) {

                try {
                    String aguiCat = tempRule.getCategoriesMapping().getAguiCategory();

                    if (!aguiCategoriesTreated.contains(tempRule.getCategoriesMapping().getCountry() + UNDERSCORE_SEPARATOR + aguiCat)) {

                        maxCategoriesToRun = getMaxCategoriesToRun(tempRule.getCategoriesMapping().getCountry(), entityType, CATEGORIES_TO_RUN_FOR);
                        maxCategoriesToCheck = getMaxCategoriesToCheck(tempRule.getCategoriesMapping().getCountry(), entityType, CATEGORIES_TO_CHECK_FOR);

                        if (categoriesToBeExecuted.size() == maxCategoriesToRun){
                            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "End: Maximum number of categories to run reached: " + maxCategoriesToRun);
                            break;
                        }

                        if (categoriesChecked == maxCategoriesToCheck) {
                            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "End: Maximum number of categories to check reached: " + maxCategoriesToCheck);
                            break;
                        }

                        // Check if some of the rules of the category are in status ERROR-AMS
                        // If so, we skip the category
                        countErrorAms = rulesServices.haveRuleSomeErrorAms(tempRule.getCategoriesMapping().getCountry(), tempRule.getCategoriesMapping().getAguiCategory());

                        if (countErrorAms != null && countErrorAms > 0) {
                            date = sdf.format(tempRule.getLastExecutionDate());
                            result = getResultForStarter(tempRule.getCategoriesMapping().getCountry(), tempRule.getEntityType(), date, aguiCat, "*",
                                    "Skipped, some rule are in ERROR-AMS...");
                            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " +
                                    tempRule.getCategoriesMapping().getCountry() + "_" + aguiCat + tempRule.getCategoriesMapping().getAinCategory() + " skipped (some rule are in ERROR-AMS");
                            resultMap.put(aguiCat + tempRule.getCategoriesMapping().getAinCategory(), result);
                            continue;
                        }

                        categoriesChecked++;

                        idsForHCAD = new HashMap<>();

                        // Before create AD/HC request we are going to align the log_compon_alert records in status NEW (because, if we have some records in NEW status for
                        // equipments not yet created in AIN, those will be missing because the new_data query is not taking it. So, if for a new equipment not created yet in AIN
                        // we have a hierarchy change, then the alignment process will set this record to DISCARDED.

                        if (!tempRule.getCategoriesMapping().getCountry().equalsIgnoreCase("BRA")) {
                            if (parametersByCountry.containsKey(tempRule.getCategoriesMapping().getCountry()))
                                parameters = parametersByCountry.get(tempRule.getCategoriesMapping().getCountry());
                            else {
                                parameters = rulesServices.getParameters(tempRule.getCategoriesMapping().getCountry(), System.getenv("ENV").toUpperCase());
                                parametersByCountry.put(tempRule.getCategoriesMapping().getCountry(), parameters);
                            }

                            boolean isSubcomponent = rulesServices.checkIfAguiCategoryHaveSubcomponents(tempRule.getCategoriesMapping().getCountry(), aguiCat);
                            // For Equipments and Subequipments, we alling first the log_compon_alert table to avoid problems with the syncronization
                            // between AUI and AGUI
                            if (entityType.equalsIgnoreCase(AINTypesEnum.EQUIPMENT.getCode()) || entityType.equalsIgnoreCase(AINTypesEnum.SUBEQ.getCode()))
                                logComponAlertTableService.alignHierarchyChanges(tempRule.getCategoriesMapping().getCountry(), aguiCat, parameters, new JsonObject(), true, isSubcomponent);

                            // First, check and create Agui Deletions
                            adResult = launchAguiDeletions(ainType, tempRule.getCategoriesMapping().getCountry(), aguiCat, logTransformator.getId(), rulesReady, categoriesWithAD, idsForHCAD);
                            result.add("aguiDeletions", adResult);
                        }

                        if (adResult != null && adResult.has("error")) {
                            checkRuleRetries(tempRule, maxRetries, logTransformator.getId(), "ISSUE: " + adResult.get("error").getAsString());
                        } else {
                            if (!tempRule.getCategoriesMapping().getCountry().equalsIgnoreCase("BRA")) {
                                // Second, check and create Hierarchy Changes
                                hcResult = launchHierachyChanges(ainType, tempRule.getCategoriesMapping().getCountry(), aguiCat, logTransformator.getId(), rulesReady,
                                        allRules.get(aguiCat + SHARP_SEPARATOR + tempRule.getCategoriesMapping().getCountry()), categoriesWithHC, maxRetries, idsForHCAD);
                                result.add("hierarchyChanges", hcResult);
                            }
                            if (hcResult != null && hcResult.has("error")) {
                                if (!hcResult.has("errorStop"))
                                    checkRuleRetries(tempRule, maxRetries, logTransformator.getId(), hcResult.get("error").getAsString());

                                // If we find an ISSUE (hc with ain category change) we avoid to continue with other rules for this agui category (in case we have a 1:n category)
                                if (hcResult.get("error").getAsString().startsWith("ISSUE")) {
                                    aguiCategoriesTreated.add(tempRule.getCategoriesMapping().getCountry() + UNDERSCORE_SEPARATOR + aguiCat);
                                    orchestrationService.deleteAllAguiCatHCADRequestWithMap(tempRule.getCategoriesMapping().getCountry(), aguiCat, idsForHCAD);
                                }
                            } else {

                                for (String ain : allRules.get(aguiCat + SHARP_SEPARATOR + tempRule.getCategoriesMapping().getCountry())) {

                                    try {

                                        rule = rulesReady.stream().filter(r ->
                                                r.getCategoriesMapping().getCountry().equalsIgnoreCase(tempRule.getCategoriesMapping().getCountry()) &&
                                                        r.getCategoriesMapping().getAguiCategory().equalsIgnoreCase(aguiCat) &&
                                                        r.getCategoriesMapping().getAinCategory().equalsIgnoreCase(ain)).findFirst();

                                        if (rule.isPresent()) {
                                            CategoriesMapping auxCatMap = rule.get().getCategoriesMapping();
                                            logTransformator.setDistributionCompany(auxCatMap.getDistributionCompany());
                                            aguiCategoriesTreated.add(auxCatMap.getCountry() + UNDERSCORE_SEPARATOR + aguiCat);

                                            if (categoriesToBeExecuted.size() != maxCategoriesToRun) {

                                                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Starting category: " + auxCatMap.getCountry() + "_" + aguiCat + ain);

                                                if (minutesPerCountry.containsKey(auxCatMap.getCountry()))
                                                    minutes = minutesPerCountry.get(auxCatMap.getCountry());
                                                else {
                                                    minutes = Integer.parseInt(parametersRepository.findByCountryAndEnvironmentAndName(auxCatMap.getCountry(),
                                                            System.getenv("ENV").toUpperCase(), "MINUTES_TO_REST_FOR_DELTA_DATE").getValue());
                                                    minutesPerCountry.put(auxCatMap.getCountry(), minutes);
                                                }

                                                dateFrom = new Date(rule.get().getLastExecutionDate().getTime() - minutes * 60 * 1000);
                                                date = sdf2.format(dateFrom);

                                                if (!tempRule.getCategoriesMapping().getCountry().equalsIgnoreCase("BRA")) {
                                                    count = aguiService.countRecordsForCountryCategoryEntityTypeAndDate(auxCatMap.getCountry(),
                                                            aguiCat, entityType, date);
                                                } else {
                                                    SimpleDateFormat sdfymd = new SimpleDateFormat("yyyy-MM-dd HH:mm:ss");
                                                    date = sdfymd.format(dateFrom);
                                                    count = dataLakeService.countRecords(auxCatMap.getAinCategory(), entityType, date);
                                                }
                                                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Records in Agui for : " + aguiCat + ain + " => " + count);

                                                if (count == -1) {
                                                    result = getResultForStarter(auxCatMap.getCountry(), rule.get().getEntityType(), date, aguiCat, null,
                                                            "Could not retrieve if there data from Agui");
                                                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Error retrieving count of records for : " + aguiCat + ain);
                                                    resultMap.put(aguiCat + ain, result);
                                                    // Deleted if there are some request for HC/AD for this category
                                                    orchestrationService.deleteHCADRequestWithMap(auxCatMap.getCountry(), aguiCat, ain, idsForHCAD);
                                                    checkRuleRetries(rule.get(), maxRetries, logTransformator.getId(), "ISSUE: Error retrieving count of records for : " + aguiCat + ain);
                                                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Deleted HC/AC request for: " + aguiCat + ain + " (error retrieving new data from Agui)");


                                                } else if (count == 0) {
                                                    result = getResultForStarter(auxCatMap.getCountry(), rule.get().getEntityType(), date, aguiCat, ain,
                                                            "No new data in Agui for this category");
                                                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "No new records in Agui for : " + aguiCat + ain);

                                                    if (idsForHCAD.containsKey(auxCatMap.getCountry() + "_" + aguiCat + ain) &&
                                                            !idsForHCAD.get(auxCatMap.getCountry() + "_" + aguiCat + ain).isEmpty()) {

                                                        if (setRuleInProgress(rule.get(), aguiCat, ain, logTransformator)) {
                                                            categoryToBeExecuted = new CategoriesMapping();
                                                            categoryToBeExecuted.setCountry(auxCatMap.getCountry());
                                                            categoryToBeExecuted.setDistributionCompany(auxCatMap.getDistributionCompany());
                                                            categoryToBeExecuted.setAguiCategory(aguiCat);
                                                            categoryToBeExecuted.setAinCategory(ain);
                                                            categoriesToBeExecuted.add(categoryToBeExecuted);
                                                            orchestrationService.activateHCADRequestWithMap(auxCatMap.getCountry(), aguiCat, ain, idsForHCAD);
                                                            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(),
                                                                    "HC/AC requests for: " + aguiCat + ain + " created (no new data for this category)");
                                                        } else {
                                                            result = getResultForStarter(auxCatMap.getCountry(), rule.get().getEntityType(), date, aguiCat, ain,
                                                                    "Skipped, currently running...");
                                                            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " +
                                                                    rule.get().getCategoriesMapping().getCountry() + "_" + aguiCat + ain + " skipped (record in semahpore for AGUI-AIN flow");
                                                            resultMap.put(aguiCat + ain, result);

                                                            // Delete HC/AD for this category, because is blocked
                                                            orchestrationService.deleteHCADRequestWithMap(auxCatMap.getCountry(), aguiCat, ain, idsForHCAD);


                                                        }
                                                    } else {
                                                        rule.get().setCurrentJobExecutionId(null);
                                                        rule.get().setCurrentExecutionStatus("COMPLETED");
                                                        rule.get().setCountRetry(null);
                                                        if (rule.get().getCurrentExecutionDate() != null)
                                                            rule.get().setLastExecutionDate(rule.get().getCurrentExecutionDate());
                                                        else
                                                            rule.get().setLastExecutionDate(DateTime.now().toDate());
                                                        rule.get().setCurrentExecutionDate(null);
                                                        rulesServices.updateRules(rule.get().getId(), rule.get());

                                                        //orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, getIdLogFortraces(), LocalDateTime.now().format(formatter) +
                                                        //      " extractionStarter: " + auxCatMap.getCountry() + "_" + auxCatMap.getAguiCategory() +
                                                        //      auxCatMap.getAinCategory() + "_" + rule.get().getEntityType() + " Status updated to: " + rule.get().getCurrentExecutionStatus() + " No new records found in AGUI. No HC/AD to do");

                                                        rulesServices.updateToReadyHierarchicalRules(auxCatMap, rule.get().getEntityType());
                                                    }

                                                } else {

                                                    List<CategoriesRunning> cr = new ArrayList<>();
                                                    boolean checkStatus = true;

                                                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " + aguiCat + ain +
                                                            " new data in Agui");

                                                    if (setRuleInProgress(rule.get(), aguiCat, ain, logTransformator)) {

                                                        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " + aguiCat + ain +
                                                                " match all the conditions to be started");

                                                        categoryToBeExecuted = new CategoriesMapping();
                                                        categoryToBeExecuted.setCountry(auxCatMap.getCountry());
                                                        categoryToBeExecuted.setDistributionCompany(auxCatMap.getDistributionCompany());
                                                        categoryToBeExecuted.setAguiCategory(aguiCat);
                                                        categoryToBeExecuted.setAinCategory(ain);
                                                        categoriesToBeExecuted.add(categoryToBeExecuted);

                                                        JsonObject resultTemp = getResultForStarter(auxCatMap.getCountry(), rule.get().getEntityType(), date, aguiCat, ain,
                                                                "Category selected to be executed");

                                                        for (String key : resultTemp.keySet()) {
                                                            result.add(key, resultTemp.get(key));
                                                        }

                                                        if (parametersByCountry.containsKey(categoryToBeExecuted.getCountry()))
                                                            parameters = parametersByCountry.get(categoryToBeExecuted.getCountry());
                                                        else {
                                                            parameters = rulesServices.getParameters(categoryToBeExecuted.getCountry(), System.getenv("ENV").toUpperCase());
                                                            parametersByCountry.put(categoryToBeExecuted.getCountry(), parameters);
                                                        }
                                                        jobId = launchSpringBatchExtraction(
                                                                categoryToBeExecuted.getCountry(),
                                                                categoryToBeExecuted.getAguiCategory(),
                                                                categoryToBeExecuted.getAinCategory(),
                                                                rule.get().getEntityType(),
                                                                categoryToBeExecuted.getDistributionCompany(),
                                                                p, checkStatus, parameters);

                                                        updateJobIdForHCAdRequest(jobId, idsForHCAD, categoryToBeExecuted.getCountry(), categoryToBeExecuted.getAguiCategory(), categoryToBeExecuted.getAinCategory());

                                                        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " + aguiCat + ain + " launcehd with Spring Batch. job_id: " + jobId);
                                                        result.addProperty("jobId", jobId);
                                                        if (jobId == -1) {
                                                            result.addProperty("extraction_error_message", "Missing parameter DEFAULT_CHUNK in Pararmeters table");
                                                            // Deleted if there are some request for HC/AD for this category
                                                            orchestrationService.deleteHCADRequestWithMap(categoryToBeExecuted.getCountry(), aguiCat, ain, idsForHCAD);
                                                        }
                                                        if (jobId == -2) {
                                                            result.addProperty("extraction_error_message", "The rule for: " + aguiCat + ain + " could not be found");
                                                            // Deleted if there are some request for HC/AD for this category
                                                            orchestrationService.deleteHCADRequestWithMap(categoryToBeExecuted.getCountry(), aguiCat, ain, idsForHCAD);
                                                        }

                                                        resultMap.put(aguiCat + ain, result);
                                                    } else {
                                                        result = getResultForStarter(auxCatMap.getCountry(), rule.get().getEntityType(), date, aguiCat, ain,
                                                                "Skipped, currently running...");
                                                        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " +
                                                                rule.get().getCategoriesMapping().getCountry() + "_" + aguiCat + ain + " skipped (record in semahpore for AGUI-AIN flow");
                                                        resultMap.put(aguiCat + ain, result);

                                                        //Delete HC/AD for this category, because is blocked
                                                        orchestrationService.deleteHCADRequestWithMap(auxCatMap.getCountry(), aguiCat, ain, idsForHCAD);

                                                    }

                                                }
                                            } else {
                                                // We have reached the maximum number of categories
                                                // Deleted if there are some request for HC/AD for this category
                                                orchestrationService.deleteHCADRequestWithMap(auxCatMap.getCountry(), aguiCat, ain, idsForHCAD);

                                            }
                                        } else {
                                            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Category: " +
                                                    tempRule.getCategoriesMapping().getCountry() + "_" + aguiCat + ain + " skipped. Not present in rulesReady list");

                                            // TODO
                                            // Delete, if there are, the AD/HC of this category, because we ha skipped it (is IN_PROGRESS on the previous level)
                                            logService.deleteADHCNotNeeded("PENDING", country, aguiCat, ain);
                                        }

                                    } catch (Exception e) {
                                        String message = "Orchestrator (starter inside loop). Unexpected error processing: " + tempRule.getCategoriesMapping().getCountry() + SHARP_SEPARATOR + aguiCat + ain +
                                                " => " + e.getMessage();
                                        log.error(message);
                                        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), message);
                                    }

                                }
                            }
                        }
                    }
                }catch(Exception e){
                    CategoriesMapping cat;
                    cat = tempRule.getCategoriesMapping();
                    String message = "Orchestrator (starter outside loop). Unexpected error processing: " + cat.getCountry() + SHARP_SEPARATOR + cat.getAguiCategory() + cat.getAinCategory() +
                            " => " + e.getMessage();
                    log.error(message);

                    logService.deleteADHCNotNeeded("PENDING", country, cat.getAguiCategory(), cat.getAinCategory());

                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), message);
                }

            }

        }

        try {

            if (categoriesToBeExecuted.isEmpty()) {

                result = new JsonObject();
                result.addProperty("message", "No categories found ready to be executed");
                results.add(result);
                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "No categories found ready to be executed");
            }

            for (JsonObject jo : resultMap.values()) {
                results.add(jo);
            }

            logTransformator.setJsonResult(results.toString());
            logTransformator.setEndDate(new Date());
            orquestrationLogService.updateLog(logTransformator);

            // Unblock entityType on semaphore
            String typeForSemaphore = getTypeForSemaphore(entityType);
            List<CategoriesRunning> cr = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow("ALL", typeForSemaphore, typeForSemaphore, "ORCHESTRATOR");
            if (cr.size() == 1)
                lockDbService.deleteRecordRunningCategory(cr.getFirst().getId());

        }catch(Exception e){
            String message = "Orchestrator (managing log messages). Unexpected error writing the final logs or unblocking the entitype on the semaphore: " + e.getMessage();
            log.error(message);

        }

        log.error("[ORCHESTRATOR] End Stater for: " + country + "_" + entityType);

        return new ResponseEntity<>(results.toString(), HttpStatus.OK);

         */
    }

    @Override
    @GetMapping(value = "/agui/starterDocuments", produces = "application/json")
    public ResponseEntity<String> extractionStarterDocuments(String country, String entityType){

        String entityTypeValue;

        try {
            AINTypesEnum ainEntityType = AINTypesEnum.valueOf(entityType);
            entityTypeValue = ainEntityType.getCode();
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body("{\"Error\": \"Wrong entityType. Allowed values\":  [" + AINTypesEnum.getValues() + "]\"}");
        }

        Map<String, List<String >> allRules;
        String date;

        long jobId;
        SimpleDateFormat sdf = new SimpleDateFormat("dd-MM-yyyy HH:mm:ss");
        Map<String, Map<String, String>> parametersByCountry = new HashMap<>();
        Map<String, String> parameters;

        mapper.disable(SerializationFeature.FAIL_ON_EMPTY_BEANS);

        Map<String ,Integer> minutesPerCountry = new HashMap<>();

        OrquestrationLog logTransformator = orquestrationLogService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
        logTransformator.setSessionName("LOCAL Documents STARTER for " + country + UNDERSCORE_SEPARATOR + entityTypeValue);
        logTransformator.setCountry(country);
        logTransformator.setEntityType(entityTypeValue);
        logTransformator.setCreationDate(new Date());
        orquestrationLogService.updateLog(logTransformator);

        if (StringUtils.isBlank(country)) {
            country = "ITA";
        } else {
            if (!country.equalsIgnoreCase("ITA")) {
                String message = "Country not allowed: " + country + ". Countries allowed: [\"ITA\"].";
                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.ERROR, logTransformator.getId(), message);
                return new ResponseEntity<>(
                        "{\"status\": \"KO\", \"error\":" + message + "}",
                        HttpStatus.BAD_REQUEST);
            }
        }

        Parameters p = parametersRepository.findByCountryAndEnvironmentAndName(COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase(), "DEFAULT_CHUNK");

        List<Object> rulesReadRs = rulesServices.getRulesReadyToBeExecutedForDocs(entityTypeValue, country);
        List<Rules> rulesReady = convertObjectsToRules(rulesReadRs);

        rulesReady.sort(Comparator.comparing(Rules::getLastDocsExecutionDate));
        allRules = getRulesMap(rulesReady, entityTypeValue);

        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(),
                "Rules ready to be executed for docs: " + rulesReady.size());

        Map<String, JsonObject> resultMap =new HashMap<>();
        int categoriesExecuted = 0;

        if (!rulesReady.isEmpty()) {
            List<String> aguiCategoriesTreated = new ArrayList<>();
            for (Rules tempRule : rulesReady) {

                String catAguiCategory = tempRule.getCategoriesMapping().getAguiCategory();
                String catAinCategory = tempRule.getCategoriesMapping().getAinCategory();
                String catCountry = tempRule.getCategoriesMapping().getCountry();
                String catDDC = tempRule.getCategoriesMapping().getDistributionCompany();

                if (!aguiCategoriesTreated.contains(catCountry + UNDERSCORE_SEPARATOR + catAguiCategory)) {

                    int maxCategories = getMaxCategoriesToRun(catCountry, entityTypeValue, CATEGORIES_TO_RUN_FOR);

                    if (categoriesExecuted == maxCategories)
                        break;

                    // Check if some of the rules of the category are in status ERROR-AMS
                    // If so, we skip the category
                    Long countErrorAms = rulesServices.haveRuleSomeErrorAmsForDocs(catCountry,
                            catAguiCategory);

                    if (countErrorAms != null && countErrorAms > 0){
                        date = sdf.format(tempRule.getLastDocsExecutionDate());
                        JsonObject result = getResultForStarter(catCountry, tempRule.getEntityType(), date, catAguiCategory, "*",
                                "Skipped, some rule are in ERROR-AMS...");
                        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " + catAguiCategory + catAinCategory + " skipped (some rule are in ERROR-AMS");
                        resultMap.put(catAguiCategory + catAinCategory, result);
                        continue;
                    }

                    Optional<Rules> optRule;
                    for (String ain : allRules.get(catAguiCategory + SHARP_SEPARATOR + catCountry)) {

                        optRule = rulesReady.stream().filter(r ->
                                r.getCategoriesMapping().getCountry().equalsIgnoreCase(catCountry) &&
                                        r.getCategoriesMapping().getAguiCategory().equalsIgnoreCase(catAguiCategory) &&
                                        r.getCategoriesMapping().getAinCategory().equalsIgnoreCase(ain)).findFirst();

                        if (optRule.isPresent()) {
                            Rules rule = optRule.get();
                            logTransformator.setDistributionCompany(rule.getCategoriesMapping().getDistributionCompany());
                            aguiCategoriesTreated.add(catCountry + UNDERSCORE_SEPARATOR + catAguiCategory);

                            if (categoriesExecuted < maxCategories) {

                                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Starting category: " + catAguiCategory + ain);
                                int minutes = 0;
                                if (minutesPerCountry.containsKey(catCountry))
                                    minutes = minutesPerCountry.get(catCountry);
                                else{
                                    minutes = Integer.parseInt(parametersRepository.findByCountryAndEnvironmentAndName(catCountry,
                                            System.getenv("ENV").toUpperCase(), "MINUTES_TO_REST_FOR_DELTA_DATE").getValue());
                                    minutesPerCountry.put(rule.getCategoriesMapping().getCountry(), minutes);
                                }
                                Date dateFrom = new Date(rule.getLastDocsExecutionDate().getTime() - minutes * 60 * 1000);
                                if (country.equalsIgnoreCase("BRA")) {
                                    SimpleDateFormat sdfymd = new SimpleDateFormat("yyyy-MM-dd HH:mm:ss");
                                    date = sdfymd.format(dateFrom);
                                } else {
                                    date = sdf.format(dateFrom);
                                }

                                int count = getCountRecords(catCountry, modelMapper.map(rule, RulesDTO.class), entityTypeValue, date);

                                orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Records in Agui for : " + catAguiCategory + ain + " => " + count);

                                if (count == -1) {
                                    JsonObject result = getResultForStarter(catCountry, rule.getEntityType(), date, catAguiCategory, null,
                                            "Could not retrieve if there data from Agui");
                                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Error retrieving count of records for : " + catAguiCategory + ain);
                                    resultMap.put(catAguiCategory + ain, result);
                                } else if (count == 0) {
                                    JsonObject result = getResultForStarter(catCountry, rule.getEntityType(), date, catAguiCategory, ain,
                                            "No new data in Agui for this category");
                                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "No new records in Agui for : " + catAguiCategory + ain);
                                    updateCompletedDocRule(rule);
                                    resultMap.put(catAguiCategory + ain, result);
                                } else {
                                    boolean checkStatus = true;
                                    orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " + catAguiCategory + ain +
                                            " new data in Agui");
                                    if (setDocsRuleInProgress(rule, catAguiCategory, ain)){
                                        JsonObject result = new JsonObject();
                                        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " + catAguiCategory + ain +
                                                " match all the conditions to be started");
                                        categoriesExecuted++;

                                        JsonObject resultTemp = getResultForStarter(catCountry, rule.getEntityType(), date, catAguiCategory, ain,
                                                "Category selected to be executed");

                                        for (String key : resultTemp.keySet()) {
                                            result.add(key, resultTemp.get(key));
                                        }

                                        if (parametersByCountry.containsKey(catCountry))
                                            parameters = parametersByCountry.get(catCountry);
                                        else {
                                            parameters = rulesServices.getParameters(catCountry, System.getenv("ENV").toUpperCase());
                                            parametersByCountry.put(catCountry, parameters);
                                        }
                                        jobId = launchSpringBatchForDocs(entityTypeValue,
                                                catCountry,
                                                catAguiCategory,
                                                ain,
                                                catDDC,
                                                null,
                                                p, parameters);

                                        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " + catAguiCategory + ain + " launcehd with Spring Batch. job_id: " + jobId);
                                        result.addProperty("jobId", jobId);
                                        if (jobId == -1) {
                                            result.addProperty("extraction_error_message", "Missing parameter DEFAULT_CHUNK in Pararmeters table");
                                        }
                                        if (jobId == -2) {
                                            result.addProperty("extraction_error_message", "The rule for: " + catAguiCategory + ain + " could not be found");
                                        }

                                        resultMap.put(catAguiCategory + ain, result);
                                    } else {
                                        JsonObject result = getResultForStarter(catCountry, rule.getEntityType(), date, catAguiCategory, ain,
                                                "Skipped, currently running...");
                                        orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Rule: " + catAguiCategory + ain + " skipped (record in semahpore for AGUI-AIN flow");
                                        resultMap.put(catAguiCategory + ain, result);
                                    }
                                }
                            }
                        }else{
                            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "Category: " + catAguiCategory + ain + " skipped. Not present in rulesReady list");
                        }
                    }
                }
            }
        }
        JsonArray results = new JsonArray();
        if (categoriesExecuted == 0){

            JsonObject result = new JsonObject();
            result.addProperty("message", "No categories found ready to be executed");
            results.add(result);
            orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, logTransformator.getId(), "No categories found ready to be executed");
        }

        for(JsonObject jo : resultMap.values()){
            results.add(jo);
        }

        logTransformator.setJsonResult(results.toString());
        logTransformator.setEndDate(new Date());
        orquestrationLogService.updateLog(logTransformator);

        return new ResponseEntity<>(results.toString(), HttpStatus.OK);
    }

    @Override
    @PutMapping(value = "/excel/electricType", produces = "application/json")
    public ResponseEntity<String> importExcelETypes(MultipartFile file, String type, String country, String distributionCompany,
                                                        Integer chunkSize, String mode){

        AINTypesEnum eType;
        try {
            eType = AINTypesEnum.valueOf(type);
        } catch (Exception e) {
            return ResponseEntity.internalServerError().body("{ \"status\": \"KO\", \"message\": \"Unknown value for type: \"" + type + "\"}");
        }

        validateExcelImport(eType, country, distributionCompany);

        try (Workbook workbook = WorkbookFactory.create(file.getInputStream())) {

            Sheet sheet = workbook.getSheetAt(0); // Obtiene la primera hoja

            Row rowName = sheet.getRow(0);
            if (rowName != null) {
                MapperCache cache = new MapperCache(rulesServices, rowName, eType, country);
                sheet.removeRow(rowName);
                switch (eType) {
                    case AINTypesEnum.CONDUCTOR_TYPE -> {
                        List<ConductorDataDTO> list = rulesServices.extractSpecialData(ConductorDataDTO.class, cache, sheet, country, distributionCompany);
                        rulesServices.putSpecialData(ConductorDataDTO.class, list, cache);
                    }
                    case AINTypesEnum.TRANSFORMER_TYPE -> {
                        List<TransformerDataDTO> list = rulesServices.extractSpecialData(TransformerDataDTO.class, cache, sheet, country, distributionCompany);
                        rulesServices.putSpecialData(TransformerDataDTO.class, list, cache);
                    }
                    case AINTypesEnum.GROUND_DEVICE_TYPE -> {
                        List<GrDeviceDataDTO> list = rulesServices.extractSpecialData(GrDeviceDataDTO.class, cache, sheet, country, distributionCompany);
                        rulesServices.putSpecialData(GrDeviceDataDTO.class, list, cache);
                    }
                    default -> throw new ValueNotAllowedException(type, new String[] {"CONDUCTOR_TYPE", "TRANSFORMER_TYPE", "GROUND_DEVICE_TYPE"});
                }

                String response = "{\"Status\": \"OK\"}";
                return ResponseEntity.ok(response);
            } else {
                return ResponseEntity.internalServerError().body("{ \"status\": \"KO\", \"message\": \"Excel file with no header row.\"}");
            }



        } catch (IOException e) {
            return ResponseEntity.internalServerError().body("{ \"status\": \"Warning\", \"message\": \"Unable to process Excel file.");
        } catch (Exception e) {
            return ResponseEntity.internalServerError().body("{ \"status\": \"KO\", \"message\": \"" + e.getMessage() + "\"}");
        }
    }


    private void validateExcelImport(AINTypesEnum type, String country, String distributionCompany) {
        if (!type.equals(AINTypesEnum.CONDUCTOR_TYPE) &&
                !type.equals(AINTypesEnum.TRANSFORMER_TYPE) &&
                !type.equals(AINTypesEnum.GROUND_DEVICE_TYPE)) {
            throw new ValueNotAllowedException(type.getCode(), new String[] {"CONDUCTOR_TYPE", "TRANSFORMER_TYPE", "GROUND_DEVICE_TYPE"});
        }
        if ("BRA".equals(country)) {
            Set<String> allowedCompanies = Set.of("CE", "BSP", "RJ");

            if (!allowedCompanies.contains(distributionCompany.toUpperCase())) {
                throw new ValueNotAllowedException(distributionCompany, allowedCompanies.toArray(new String[0]));
            }
        } else {
            throw new ValueNotAllowedException(distributionCompany, new String[]{"BRA"});
        }
    }

    @Override
    @GetMapping(value = "/dbrules/extract", produces = "application/json")
    public ResponseEntity<String> startBatchETypesToAIN(String country, String ainCategory, String type,
                                                        String distributionCompany, String actionType)  {

        log.info("Begin dbRules extraction.");

        Long jobId = 0L;

        AINTypesEnum eType;
        try {
            eType = AINTypesEnum.valueOf(type);
        } catch (Exception e) {
            return ResponseEntity.badRequest().body("{ \"status\": \"KO\", \"message\": \"Unknown value for type: \"" + type + "\"}");
        }
        if (!actionType.equals("C") && !actionType.equals("U")) {
            return ResponseEntity.badRequest().body("{ \"status\": \"KO\", \"message\": \"Allowed values for actionType: ['C', 'U']}");
        }

        validateExcelImport(eType, country, distributionCompany);

        jobId = extractDBRules(country, ainCategory, type, distributionCompany, actionType);

        log.info("End dbRules extraction.");
        String response =  String.format("{ \"status\": \"ok\", \"job_execution_id\" : %.0f }", jobId.floatValue());
        return ResponseEntity.ok(response);
    }

    /**
     * In case the rule has finished correctly, it gets updated
     * @param rule the rule to be finished
     */
    private void updateCompletedDocRule(Rules rule) {
        rule.setCurrentJobExecutionId(null);
        rule.setCurrentDocsExecutionStatus("COMPLETED");
        rule.setCountRetry(null);
        if (rule.getCurrentDocsExecutionDate() != null)
            rule.setLastDocsExecutionDate(rule.getCurrentDocsExecutionDate());
        else
            rule.setLastExecutionDate(DateTime.now().toDate());
        rule.setCurrentExecutionDate(null);
        rulesServices.updateRules(rule.getId(), rule);
    }

    /**
     * Save per country in a map the max of categories to be executed by orquestrator
     * @param country code of the country
     * @param entityTypeValue name of the entity
     * @return the number of the categories to be executed per time
     */
    private int getMaxCategoriesToRun(String country, String entityTypeValue, String parameterName) {
        if (maxCategoriesToRunPerCountry.containsKey(country))
            return maxCategoriesToRunPerCountry.get(country);
        else {
            Integer maxCategories = getMaxNumberOfCategoriesToExtract(country, entityTypeValue, parameterName);
            maxCategoriesToRunPerCountry.put(country, maxCategories);
            return maxCategories;
        }
    }

    /**
     * Count all the records that will migrate the job
     * @param country code of the country
     * @param rule record of the rule
     * @param entityTypeValue name of the entity
     * @param date date from to start to count
     * @return number of rows
     */
    private int getCountRecords(String country, RulesDTO rule, String entityTypeValue, String date) {
        int count = 0;
        if (!country.equalsIgnoreCase("BRA")) {
            //TODO: values for fullmigration and mode
            boolean isFullMigration = true;
            String mode = CREATION_MODE;
            count = auiService.countRecordsForDocs(rule, isFullMigration,
                    mode, rule.getEntityType(), date);
        } else {
            count = dataLakeService.countRecords(rule.getCategoriesMapping().getAinCategory(), entityTypeValue, date);
        }
        return count;
    }

    private static JsonObject getResultForStarter(String country, String entityType, String date, String aguiCat, String ainCat, String message) {
        JsonObject result;
        result = new JsonObject();
        result.addProperty("country", country);
        result.addProperty("aguiCategory", aguiCat);
        if (ainCat != null)
            result.addProperty("ainCategory", ainCat);
        result.addProperty("entityType", entityType);
        result.addProperty("date", date);
        result.addProperty("message", message);
        return result;
    }

    private List<Rules> convertObjectsToRules(List<Object> rulesReadRs){

        List<Rules> rules = new ArrayList<>();
        Object[] row;
        Rules rule;
        CategoriesMapping cm;
        Subcomponents subcomp;

        for(Object o : rulesReadRs){

            rule = new Rules();
            cm = new CategoriesMapping();
            row = (Object[])o;
            rule.setId((Integer)row[0]);
            rule.setAguiQuerySelect((String)row[1]);
            rule.setEntityType((String)row[2]);
            rule.setImage((String)row[4]);
            rule.setAguiQueryFrom((String)row[5]);
            rule.setAguiQueryGroup((String)row[6]);
            rule.setAguiQueryOrder((String)row[7]);
            rule.setLastCorrectExtraction((Date)row[8]);
            rule.setNextExtraction((Date)row[9]);
            rule.setTipoComp((String)row[10]);
            rule.setEnableCreation((Boolean) row[11]);
            rule.setCurrentJobExecutionId((Integer)row[12]);
            rule.setLastExecutionDate((Date)row[13]);
            rule.setCurrentExecutionDate((Date)row[14]);
            rule.setCurrentExecutionStatus((String)row[15]);
            rule.setCountRetry((Integer)row[16]);
            rule.setDependency((String)row[17]);
            rule.setCurrentDocsExecutionStatus((String)row[20]);
            rule.setLastDocsExecutionDate((Date)row[18]);
            rule.setCurrentDocsExecutionDate((Date)row[19]);
            rule.setLastErrorDate((Date)row[21]);

            cm.setId((Integer)row[22]);
            cm.setCountry((String)row[23]);
            cm.setAinCategory((String)row[24]);
            cm.setAguiCategory((String)row[25]);
            cm.setSpecial((Boolean)row[26]);
            cm.setDistributionCompany((String)row[27]);

            subcomp = new Subcomponents();
            if (row[28] != null) {
                subcomp.setId((Integer) row[28]);
                if (row[29] != null)
                    subcomp.setType((String) row[29]);
                if (row[30] != null)
                    subcomp.setDescription((String) row[30]);
                if (row[31] != null)
                    subcomp.setTypeHierarchy((String) row[31]);
                if (row[32] != null)
                    subcomp.setModelHierarchy((String) row[32]);
                if (row[33] != null)
                    subcomp.setEqHierarchy((String) row[33]);
                cm.setSubcomponent(subcomp);
            }else
                cm.setSubcomponent(null);

            rule.setCategoriesMapping(cm);
            rules.add(rule);

        }

        return rules;

    }

    private long launchSpringBatchForDocs(String type, String country, String aguiCategory, String ainCategory,
                                             String distributionCompany, String actionType, Parameters p,
                                             Map<String,String> parameters) {

        Long jobId;
        Optional<Rules> rule;
        boolean skipXml;

        if (p != null){
            rule = rulesServices.getRule(type,
                    country, ainCategory, aguiCategory, distributionCompany);
            if (rule.isPresent()) {
                jobId = extractAguiDocuments(type, country, aguiCategory, ainCategory, distributionCompany, null, null, Integer.parseInt(p.getValue()), actionType,-1, -1, EQ_DOCS_MODE, null);
                //orchestrationService.updateRuleData(rule.get(), jobId, type);
            }else
                jobId = -2L;
        }else{
            jobId = -1L;
        }
        return jobId;
    }

    private int getMaxNumberOfCategoriesToExtract(String country, String type, String parameter){

        int maxNumber = 10;
        String parameterName= "";
        Parameters p;
        AINTypesEnum ainType;

        ainType = AINTypesEnum.valueOf(type);

        switch (ainType){
            case GLOBAL_TYPE:
            case SUBTYPE:
            case LOCAL_TYPE:
            case SUBCOMP_TYPE:
                parameterName = parameter + "TYPES";
                maxNumber = 10;
                break;
            case OEM_MODEL:
            case SUBCOMP_DSHEET:
                parameterName = parameter + "MODELS";
                maxNumber = 10;
                break;
            case EQUIPMENT:
            case SUBEQ:
                parameterName = parameter + "EQUIPMENTS";
                maxNumber = 5;
                break;

        }

        p = parametersRepository.findByCountryAndEnvironmentAndName(country, System.getenv("ENV").toUpperCase(), parameterName);

        if (p != null)
            maxNumber = Integer.parseInt(p.getValue());
        else{
            log.error("Parameter: " + parameterName + " not found in parameters table. Using default value: " + maxNumber);
        }

        return maxNumber;

    }

    private Map<String, List<String>> getRulesMap( List<Rules> rulesReady, String type){

        List<String> ainCategories;
        Map<String, List<String>> result = new HashMap<>();
        Set<String> aguiCategoriesSet = new HashSet<>();
        String aguiCat;
        String country;
        List<String> entitiesType = rulesServices.getListOfEntiies(type);

        for (Rules r : rulesReady){
            aguiCategoriesSet.add(r.getCategoriesMapping().getAguiCategory() + SHARP_SEPARATOR + r.getCategoriesMapping().getCountry());
        }

        for (String aguiCategory : aguiCategoriesSet){
            aguiCat = aguiCategory.split(SHARP_SEPARATOR)[0];
            country = aguiCategory.split(SHARP_SEPARATOR)[1];
            if (!result.containsKey(aguiCategory)) {
                ainCategories = getAinCategoriesList(aguiCat, country, entitiesType);
                result.put(aguiCategory, ainCategories);
            }
        }

        return result;

    }

    private List<String> getAinCategoriesList(String aguiCategory, String country, List<String> entitiesType){

        return rulesServices.getAinCategoriesListByAguiCategoryAndCountryAndEntityType(aguiCategory, country, entitiesType);

    }

    private void discardAllRequestOfJob(int jobId, int ruleId){

        List<CategoriesRunning> cr;
        Optional<Rules> rule;

        logService.updateStatusForJobId("DISCARDED",jobId);

        rule = rulesServices.getRuleById(ruleId);

        if (rule.isPresent()){
            rule.get().setCurrentJobExecutionId(null);
            rule.get().setCurrentExecutionStatus("ERROR-AMS");
            rulesServices.updateRules(ruleId, rule.get());

            CategoriesMapping catMapAux = rule.get().getCategoriesMapping();

            //orquestrationLogService.addNewMessagelog(LogMessageTypeEnum.INFO, getIdLogFortraces(), LocalDateTime.now().format(formatter) +
            //      " extractionStarter: " + catMapAux.getCountry() + "_" + catMapAux.getAguiCategory() +
            //      catMapAux.getAinCategory() + "_" + rule.get().getEntityType() + " Status updated to: " + rule.get().getCurrentExecutionStatus() + " No new records found in AGUI. No HC/AD to do");

            // Delete the record in the Semaphore
            cr = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow(rule.get().getCategoriesMapping().getCountry(),
                    rule.get().getCategoriesMapping().getAguiCategory(), rule.get().getCategoriesMapping().getAinCategory(), "AGUI-TO-AIN");

            if (!cr.isEmpty() && cr.size() == 1)
                lockDbService.deleteRecordRunningCategory(cr.get(0).getId());
        }


    }



    private JsonObject checkRequestRetries(Rules rule, int maxRetries, LogDTO l){

        List<CategoriesRunning> cr;
        Integer retries = l.getCountRetry();
        String retryResult;
        JsonObject result = null;
        boolean doRetry = false;

        if (retries == null) {
            l.setCountRetry(1);
            //rule.setCurrentExecutionStatus("TO-RETRY");
            doRetry = true;

        }
        else if (retries == maxRetries) {

            l.setMigrationStatus("DISCARDED");
            logService.updateLog(new ObjectMapper().convertValue(l, LogDTO.class));

            rule.setCurrentJobExecutionId(null);
            rule.setCurrentExecutionStatus("ERROR-AMS");
            result = new JsonObject();
            result.addProperty("status", "ko");
            result.addProperty("error", "Discarded, to many retries");

            // Delete the record in the Semaphore
            cr = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow(rule.getCategoriesMapping().getCountry(),
                    rule.getCategoriesMapping().getAguiCategory(), rule.getCategoriesMapping().getAinCategory(), "AGUI-TO-AIN");

            if (!cr.isEmpty() && cr.size() == 1)
                lockDbService.deleteRecordRunningCategory(cr.get(0).getId());
        }
        else {
            l.setCountRetry(retries + 1);
            //rule.setCurrentExecutionStatus("TO-RETRY");
            doRetry = true;
        }

        if (doRetry){
            retryResult = processRetry(l, "", true);
            result = JsonParser.parseString(retryResult).getAsJsonObject();
            if (result.has("status") && result.get("status").getAsString().equalsIgnoreCase("ko"))
                result.addProperty("request_id", l.getRequestId());
        }

        return result;

    }

    @Override
    @GetMapping(value = "/agui/extract", produces = "application/json")
    public String startBatchAgui(String country, String aguiCategory, String ainCategory, String type,
                                 String distributionCompany, String dateFrom, String dateTo, Integer chunkSize,
                                 boolean forceExtraction, long skipRecords, long numberOfBlocks, boolean skipXML, String actionType, String matricolas) {
        log.info("Begin startBatchAgui");

        String category;

        Long jobId = 0L;

        category = aguiCategory + ainCategory;

        Parameters p = parametersRepository.findByCountryAndEnvironmentAndName(COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase(), "MAX_CHUNK");

        if (p != null && chunkSize <= Integer.parseInt(p.getValue())){

            if (validateDates(dateFrom, dateTo)) {

                Integer countInProgress = extractorService.getCountInProgressExecution(ExtractorConstants.STEP_AGUI);
                Boolean isLastExecution = extractorService.getExecutionLast24Hours(country, category, ExtractorConstants.AGUI_EQUIPMENT);
                Boolean isCurrentExecution = extractorService.getCurrentExecution(country, category, ExtractorConstants.AGUI_EQUIPMENT);
                if (countInProgress < defaultCountInProgress) {
                    if ((!isLastExecution && !isCurrentExecution) || forceExtraction) {
                        jobId = extractAgui(country, aguiCategory, ainCategory, type, distributionCompany, dateFrom, dateTo, chunkSize, skipRecords, numberOfBlocks,
                                skipXML, FULL_MIGRATION_MODE, matricolas, actionType);
                    } else {
                        log.info("Nothing to extract for : " + category + " in: " + country);
                    }
                } else {
                    log.error(msgError.replace("$", String.valueOf(defaultCountInProgress)));
                    throw new TooManyRequestException(msgError.replace("$", String.valueOf(defaultCountInProgress)));
                }
            } else {

                return "{\"status\": \"ko\", \"error\": \"Dates must be in format YYYY-MM-DD HH24:MI:SS\"}";

            }
        }else{
            if (p != null)
                return "{\"status\": \"ko\", \"error\": \"Maximum value for chunkSize parameter: " + p.getValue() + "\"}";
            else
                return "{\"status\": \"ko\", \"error\": \"MAX_CHUNK parameter not found in the rules db\"}";
        }

        log.info("End startBatchAgui");
        return String.format("{ \"status\": \"ok\", \"job_execution_id\" : %.0f }", jobId.floatValue());

    }

    @Override
    @GetMapping(value = "/agui/extractXmlProperties", produces = "application/json")
    public String startBatchAguiXmlProperties(String country, String aguiCategory, String ainCategory, String dateFrom,
                                              String dateTo, Integer chunkSize, boolean forceExtraction,
                                              long skipRecords,  long numberOfBlocks) {
        log.info("Begin startBatchAguiXmlProperties");

        String category;

        Long jobId = 0L;

        category = aguiCategory + ainCategory;

        Parameters p = parametersRepository.findByCountryAndEnvironmentAndName(COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase(), "MAX_CHUNK");

        if (p != null && chunkSize <= Integer.parseInt(p.getValue())){

            if (validateDates(dateFrom, dateTo)) {

                Integer countInProgress = extractorService.getCountInProgressExecution(ExtractorConstants.STEP_AGUI);
                Boolean isLastExecution = extractorService.getExecutionLast24Hours(country, category, ExtractorConstants.AGUI_EQUIPMENT);
                Boolean isCurrentExecution = extractorService.getCurrentExecution(country, category, ExtractorConstants.AGUI_EQUIPMENT);
                if (countInProgress < defaultCountInProgress) {
                    if ((!isLastExecution && !isCurrentExecution) || forceExtraction) {
                        jobId = extractAgui(country, aguiCategory, ainCategory, "EQUIPMENT", null, dateFrom, dateTo, chunkSize, skipRecords, numberOfBlocks, false, ONLY_XML_MODE, null, null);
                    } else {
                        log.info("Nothing to extract for : " + category + " in: " + country);
                    }
                } else {
                    log.error(msgError.replace("$", String.valueOf(defaultCountInProgress)));
                    throw new TooManyRequestException(msgError.replace("$", String.valueOf(defaultCountInProgress)));
                }
            } else {

                return "{\"status\": \"ko\", \"error\": \"Dates must be in format YYYY-MM-DD HH24:MI:SS\"}";

            }
        }else{
            if (p != null)
                return "{\"status\": \"ko\", \"error\": \"Maximum value for chunkSize parameter: " + p.getValue() + "\"}";
            else
                return "{\"status\": \"ko\", \"error\": \"MAX_CHUNK parameter not found in the rules db\"}";
        }

        log.info("End startBatchAgui");
        return String.format("{ \"status\": \"ok\", \"job_execution_id\" : %.0f }", jobId.floatValue());

    }

    @Override
    @GetMapping(value = "/agui/extractCreation", produces = "application/json")
    public ResponseEntity<Object> startBatchAguiCreation(String country, String aguiCategory, String ainCategory, String type, String distributionCompany, String dateFrom, String dateTo,
                                           Integer chunkSize, boolean forceExtraction, long skipRecords, long numberOfBlocks) throws IOException {

        return startBatchAguiCreationOrUpdate(country, aguiCategory, ainCategory, type, distributionCompany, dateFrom, dateTo, chunkSize, forceExtraction, skipRecords, numberOfBlocks, CREATION_MODE);

    }

    @Override
    @GetMapping(value = "/agui/extractUpdate", produces = "application/json")
    public ResponseEntity<Object> startBatchAguiUpdate(String country, String aguiCategory, String ainCategory, String type, String distributionCompany, String dateFrom, String dateTo,
                                                         Integer chunkSize, boolean forceExtraction, long skipRecords, long numberOfBlocks) throws IOException {
        AINTypesEnum ainType;

        ainType = AINTypesEnum.valueOf(type);

        if (ainType == AINTypesEnum.GLOBAL_TYPE) {
            ErrorResponseTO errorResponse = new ErrorResponseTO("Update not allowed for Global Types");
            return new ResponseEntity<>(errorResponse, HttpStatus.BAD_REQUEST);
        }
        else
            return startBatchAguiCreationOrUpdate(country, aguiCategory, ainCategory, type, distributionCompany, dateFrom, dateTo, chunkSize, forceExtraction, skipRecords, numberOfBlocks, UPDATE_MODE);

    }

    @Override
    @GetMapping(value = "/agui/extractDocuments", produces = "application/json")
    public ResponseEntity<String> startBatchAguiDocuments(String country, String aguiCategory, String ainCategory,
                                          String type, String actionType, String distributionCompany){
        String entityTypeValue;
        try {
            AINTypesEnum ainEntityType = AINTypesEnum.valueOf(type);
            entityTypeValue = ainEntityType.getCode();
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body("{\"Error\": \"Wrong entityType. Allowed values\":  [" + AINTypesEnum.getValues() + "]\"}");
        }

        log.info("Begin startBatchAguiDocuments");

        String category;

        Long jobId = 0L;

        category = aguiCategory + ainCategory;

        Parameters p = parametersRepository.findByCountryAndEnvironmentAndName(COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase(), "MAX_CHUNK");

        Integer countInProgress = extractorService.getCountInProgressExecution(ExtractorConstants.STEP_AGUI);
        Boolean isLastExecution = extractorService.getExecutionLast24Hours(country, category, ExtractorConstants.AGUI_EQUIPMENT);
        Boolean isCurrentExecution = extractorService.getCurrentExecution(country, category, ExtractorConstants.AGUI_EQUIPMENT);
        if (countInProgress < defaultCountInProgress) {
            if (!isLastExecution && !isCurrentExecution) {
                Map<String, String> parameters = rulesServices.getParameters(country, System.getenv("ENV").toUpperCase());
                jobId = launchSpringBatchForDocs(entityTypeValue,
                        country,
                        aguiCategory,
                        ainCategory,
                        distributionCompany,
                        actionType,
                        p, parameters);
            } else {
                log.info("Nothing to extract for : " + category + " in: " + country);
            }
        } else {
            log.error(msgError.replace("$", String.valueOf(defaultCountInProgress)));
            throw new TooManyRequestException(msgError.replace("$", String.valueOf(defaultCountInProgress)));
        }

        log.info("End startBatchAgui");
        return ResponseEntity.ok().body(String.format("{ \"status\": \"ok\", \"job_execution_id\" : %.0f }", jobId.floatValue()));
    }


    @Override
    @GetMapping(value = "/agui/hierarchyChange", produces = "application/json")
    public ResponseEntity<Object> startAguiHierarchyChange(String country, String aguiCategory, String ainCategory, String type,String distributionCompany) throws IOException{

        String query;
        boolean cacheOk;
        JdbcTemplate jTemp = new JdbcTemplate();
        JsonObject dataLoader = new JsonObject();
        JsonArray[] creation;
        Map<String, String> parameters;
        ObjectMapper mapper = new ObjectMapper();
        List<RecordDTO> records;
        Map<String, String> allLocalTypes = new HashMap<>();
        Map<String, String> allOems = new HashMap<>();
        Set<String> allGlobalTypes = new HashSet<>();
        Map<String, String> allEquipmentsAinObjectsIds = new HashMap<>();
        String externalId;
        Log logTransformator;
        String requestId;
        String reducedType = "";
        int fakeJobId;
        AINTypesEnum ainType;

        ainType = AINTypesEnum.valueOf(type);

        //
        int min = 0;
        int max = 99999;
        fakeJobId = (int)(Math.random()*(max-min+1)+min);

        if (ainType == AINTypesEnum.GLOBAL_TYPE || ainType == AINTypesEnum.SUBTYPE || ainType == AINTypesEnum.LOCAL_TYPE || ainType == AINTypesEnum.SUBCOMP_TYPE){

            String errorMessage = String.format("%s not allowed for hierarchy chagnged", ainType.getCode());
            return new ResponseEntity<>("{ \"status\": \"kO\", \"error\": \"" + errorMessage + "\"}", HttpStatus.PRECONDITION_FAILED);
        }


        List<CategoriesMapping> categories = rulesServices.getExtraccion(country, aguiCategory, ainCategory, ainType.getCode());
        CategoriesMapping categoryMapping = categories.get(0);

        if (categoryMapping.getRules().size() == 0){
            return new ResponseEntity<>("{ \"message\": \"No rule found in DB for this category, country and type\"}", HttpStatus.PRECONDITION_FAILED);
        }

        parameters = rulesServices.getParameters(country, System.getenv("ENV").toUpperCase());

        switch(ainType){
            case EQUIPMENT:
                reducedType = "EQ";
                break;
            case SUBEQ:
                reducedType = "SEQ";
                break;
            case OEM_MODEL:
                reducedType = "OEM";
                break;
            case SUBCOMP_DSHEET:
                reducedType = "DSH";
                break;

        }

        if (CollectionUtils.isNotEmpty(categories)) {
            query = aguiService.getQueryString(mapper.convertValue(categories.get(0), CategoriesMappingDTO.class), ainType, true, false, HIERARCHY_CHANGE_MODE, false);

           DataSource ds = ((CountryRoutingDataSource) multiRoutingDataSource).getResolvedDataSources().get(country);
            jTemp.setDataSource(ds);
            jTemp.setFetchSize(5000);
            List<AguiColumnsRecordDTO> result = jTemp.query(query,  new AguiRecordColumnsMapper());

            if (CollectionUtils.isEmpty(result)){
                logTransformator = logService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
                String sessionName ;
                SimpleDateFormat sdf = new SimpleDateFormat("dd-MM-yyyy-HH-mm-ss");
                String formattedDate = sdf.format(new Date());

                sessionName = country + UNDERSCORE_SEPARATOR + "HC_" +  aguiCategory + UNDERSCORE_SEPARATOR + ainCategory + UNDERSCORE_SEPARATOR + reducedType + UNDERSCORE_SEPARATOR + formattedDate;
                orchestrationService.addNewLog(
                        logTransformator, sessionName,
                        "NO_DATA",
                        categories.get(0).getRules().iterator().next().getId(),
                        dataLoader.toString(),
                        ainType.getCode(),
                        country,
                        null,
                        HIERARCHY_CHANGE_MODE,
                        aguiCategory,
                        ainCategory,
                        distributionCompany
                );

            }

            // For every record with hierarchy change
             for (AguiColumnsRecordDTO record : result) {

                logTransformator = logService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
                records = new ArrayList<>();
                records.add(record);


                cacheOk = orchestrationService.initializeCacheData(record, ainType.getCode(), aguiCategory, ainCategory, ainCategory, parameters, logTransformator, String.valueOf(fakeJobId), false);

                if (cacheOk) {

                    jsonForMongo.setIdLog(logTransformator.getId(), String.valueOf(fakeJobId));
                    //jsonForDataLoader.createExtractionElement(dataLoader, categories.get(0).getTemplates().iterator(), ainType.getCode());
                    try {

                        creation = jsonForMongo.createCreationElementColumnsMode(Optional.empty(), records, categoryMapping, parameters,
                                ainType.getCode(), ainCategory, aguiCategory, country, distributionCompany, HIERARCHY_CHANGE_MODE, String.valueOf(fakeJobId));
                        //dataLoader.add("creation", creation);
                        externalId = JsonForMongo.getExtId(record, ainCategory, ainType);
                        //dataLoader.add("request", jsonForDataLoader.createRequestElement(externalId, reducedType, country,
                        //        categories.get(0).getRules().iterator().next().getId(), "U", HIERARCHY_CHANGE_MODE));

                        // TODO
                        // Write in the log the request

                         if (!creation[0].isEmpty()) {
                            requestId = "I_" + UUID.randomUUID().toString().replace("-", "");
                        } else if (!creation[1].isEmpty())
                            requestId = "U_" + UUID.randomUUID().toString().replace("-", "");
                        else
                            requestId = "";

                       //requestId = orchestrationService.callStagginDbMs(dataLoader.toString(), country, aguiCategory, ainCategory, ainType.getCode());

                        JsonElement el  = dataLoader.get("request");
                        String sessionName = el.getAsJsonObject().get("sessionname").toString();
                        orchestrationService.addNewLog(
                                logTransformator,
                                sessionName,
                                requestId,
                                categories.get(0).getRules().iterator().next().getId(),
                                dataLoader.toString(),
                                ainType.getCode(),
                                country,
                                record.getActionId(),
                                HIERARCHY_CHANGE_MODE,
                                aguiCategory,
                                ainCategory,
                                distributionCompany
                        );

                    } catch (InvocationTargetException e) {
                        throw new RuntimeException(e);
                    } catch (IllegalAccessException e) {
                        throw new RuntimeException(e);
                    }
                }else{
                    String sessionName ;
                    SimpleDateFormat sdf = new SimpleDateFormat("dd-MM-yyyy-HH-mm-ss");
                    String formattedDate = sdf.format(new Date());
                    externalId = JsonForMongo.getExtId(record, ainCategory, ainType);
                    sessionName = "HC_" + country + UNDERSCORE_SEPARATOR +  externalId + UNDERSCORE_SEPARATOR + reducedType + UNDERSCORE_SEPARATOR + formattedDate;
                    orchestrationService.addNewLog(
                            logTransformator,
                            sessionName,
                            "CACHE_ERROR",
                            categories.get(0).getRules().iterator().next().getId(),
                            dataLoader.toString(),
                            ainType.getCode(),
                            country,
                            null,
                            HIERARCHY_CHANGE_MODE,
                            aguiCategory,
                            ainCategory,
                            distributionCompany
                    );
                }

            }

        }

        return new ResponseEntity<>("{ \"status\": \"ok\"}", HttpStatus.OK);

    }

    @Override
    @DeleteMapping(value = "/agui/aguiDeletion", produces = "application/json")
    public ResponseEntity<Object> startAguiDelete(@RequestParam String country, @RequestParam(required = true) String aguiCategory, @RequestParam(required = true) String type, @RequestParam(required = false) String distributionCompany) throws IOException {

        String query;
        JdbcTemplate jTemp = new JdbcTemplate();
        JsonArray creation;
        Map<String, String> parameters;
        ObjectMapper mapper = new ObjectMapper();
        List<ResponseValue> records;
        ResponseValue record;
        AguiColumnsRecordDTO recordDto;
        Log logTransformator;
        String requestId;
        String extIdOrg;
        String extId;
        String extSysForLocalType;
        Map<String, String> externalData;
        Map<String, String> externalDataOemModel = new HashMap<>();
        List<ResponseValue> lista;
        JsonObject dataLoaderDeletions = new JsonObject();
        String ainCategory;
        Map.Entry<String,String> entry;
        Map.Entry<String,String> entryOem;
        AINTypesEnum ainType;

        ainType = AINTypesEnum.valueOf(type);


        if (ainType == AINTypesEnum.GLOBAL_TYPE){

            String errorMessage = String.format("%s not allowed to delete", ainType.getCode());
            return new ResponseEntity<>("{ \"status\": \"kO\", \"error\": \"" + errorMessage + "\"}", HttpStatus.PRECONDITION_FAILED);
        }

        parameters = rulesServices.getParameters(country, System.getenv("ENV").toUpperCase());

        query = orchestrationService.buildAguiDeleteQuery(ainType.getCode(), aguiCategory, parameters);

        if (StringUtils.isNotBlank(query)){

            DataSource ds = ((CountryRoutingDataSource) multiRoutingDataSource).getResolvedDataSources().get(country);
            jTemp.setDataSource(ds);
            jTemp.setFetchSize(5000);
            List<LogComponTableDto> result = jTemp.query(query,  new LogComponAlertMapper());

            if (CollectionUtils.isEmpty(result)){
                logTransformator = logService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
                String sessionName ;
                SimpleDateFormat sdf = new SimpleDateFormat("dd-MM-yyyy-HH-mm-ss");
                String formattedDate = sdf.format(new Date());
                sessionName = "AD_" + country + UNDERSCORE_SEPARATOR + aguiCategory + UNDERSCORE_SEPARATOR +  ainType.getCode() + UNDERSCORE_SEPARATOR + formattedDate;
                orchestrationService.addNewLog(
                        logTransformator,
                        sessionName,
                        "NO_DATA",
                        -1,
                        dataLoaderDeletions.toString(),
                        ainType.getCode(),
                        country,
                        null,
                        AGUI_DELETE_MODE,
                        aguiCategory,
                        "",
                        distributionCompany
                );

            }

            // For every record deleted in Agui
            for (LogComponTableDto r : result) {

                logTransformator = logService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
                records = new ArrayList<>();
                record = new ResponseValue();
                recordDto = new AguiColumnsRecordDTO();
                recordDto.setCatComp((aguiCategory));
                recordDto.setTipoComp(r.getTipoComp());
                recordDto.setProgrCostr(r.getProgrCostr());
                recordDto.setProgrMod(r.getProgrMod());
                recordDto.setMatricola(r.getMatricola());

                extIdOrg = JsonForMongo.getExtId(recordDto, null, ainType) + SHARP_SEPARATOR;
                extSysForLocalType = JsonForMongo.getExtSys(parameters, ainType.getCode());

                // For equipments, we need to retireve too the Oem model internal id
                if (ainType == AINTypesEnum.EQUIPMENT || ainType == AINTypesEnum.SUBEQ) {

                    List<String> equipments = new ArrayList<>();
                    equipments.add(extIdOrg);

                    if (ainType == AINTypesEnum.EQUIPMENT) {
                        externalData = mongoService.getEquipmentsInternalIdComposite(equipments,
                                parameters.getOrDefault(PARAMETER_MAX_RETRIES_AIN_COMPOSITE_CALL, "5"),
                                parameters.getOrDefault(PARAMETER_SECONDS_BETWEEN_RETRIES, "3"),
                                parameters.get(PAR_EXT_SYS_EQU), true, false);
                        extSysForLocalType = JsonForMongo.getExtSys(parameters, AINTypesEnum.OEM_MODEL.getCode());
                        extId = JsonForMongo.getExtId(recordDto, null, AINTypesEnum.OEM_MODEL);
                    }else{
                        externalData = mongoService.getEquipmentsInternalIdComposite(equipments,
                                parameters.getOrDefault(PARAMETER_MAX_RETRIES_AIN_COMPOSITE_CALL, "5"),
                                parameters.getOrDefault(PARAMETER_SECONDS_BETWEEN_RETRIES, "3"),
                                parameters.get(PAR_EXT_SYS_SUBEQU), true, false);
                        extSysForLocalType = JsonForMongo.getExtSys(parameters, AINTypesEnum.SUBCOMP_DSHEET.getCode());
                        extId = JsonForMongo.getExtId(recordDto, null, AINTypesEnum.SUBCOMP_DSHEET);
                    }

                    externalDataOemModel = mongoService.getAllInternalIdComposite(aguiCategory, "", extId, true, extSysForLocalType, false);

                } else {

                    // Call externalData to get ainObjectId and internalId
                    externalData = mongoService.getAllInternalIdComposite(aguiCategory, "", extIdOrg, true, extSysForLocalType, false);

                }

                if (externalData.size() == 1) {

                    entry = externalData.entrySet().iterator().next();
                    record = new ResponseValue();
                    ainCategory = UNDERSCORE_SEPARATOR + entry.getKey().split(SHARP_SEPARATOR)[1];

                    if ((ainType == AINTypesEnum.EQUIPMENT || ainType == AINTypesEnum.SUBEQ) && MapUtils.isNotEmpty(externalDataOemModel) &&
                            externalDataOemModel.containsKey(extIdOrg +SHARP_SEPARATOR + ainCategory.replace(UNDERSCORE_SEPARATOR, ""))) {
                        record.setModelName(externalDataOemModel.get(extIdOrg +SHARP_SEPARATOR + ainCategory.replace(UNDERSCORE_SEPARATOR, "")));
                    }

                    record.setInternalId(entry.getValue());

                    lista = new ArrayList<>();
                    lista.add(record);

                    dataLoaderDeletions = new JsonObject();
                    jsonForDeletions.setIdLog(logTransformator.getId());
                    jsonForDeletions.createExtractionElement(dataLoaderDeletions, ainType, ainCategory);
                    creation = jsonForDeletions.createCreationElement(lista, ainType);
                    if (creation.size() > 0) {
                        dataLoaderDeletions.add("creation", creation);
                        dataLoaderDeletions.add("request", jsonForDeletions.createRequestElement(ainCategory + "_" + extIdOrg, ainType,  country + "_AD", distributionCompany,true));

                        requestId = orchestrationService.callStagginDbMs(dataLoaderDeletions.toString(), country, aguiCategory, ainCategory, ainType.getCode());

                        JsonElement el = dataLoaderDeletions.get("request");
                        String sessionName = el.getAsJsonObject().get("sessionname").toString();
                        orchestrationService.addNewLog(
                                logTransformator,
                                sessionName,
                                requestId,
                                -1,
                                dataLoaderDeletions.toString(),
                                ainType.getCode(),
                                country,
                                r.getActionId() ,
                                AGUI_DELETE_MODE,
                                aguiCategory,
                                ainCategory,
                                distributionCompany
                        );
                    } else {
                        logTransformator = logService.addNewLog(LogStatusTypeEnum.PROCESSING, null);
                        String sessionName;
                        SimpleDateFormat sdf = new SimpleDateFormat("dd-MM-yyyy-HH-mm-ss");
                        String formattedDate = sdf.format(new Date());
                        sessionName = country + "_AD" + UNDERSCORE_SEPARATOR + aguiCategory + UNDERSCORE_SEPARATOR + ainCategory + UNDERSCORE_SEPARATOR + ainType.getCode() + UNDERSCORE_SEPARATOR + formattedDate;
                        orchestrationService.addNewLog(
                                logTransformator,
                                sessionName,
                                "NO_DATA",
                                -1,
                                dataLoaderDeletions.toString(),
                                ainType.getCode(),
                                country,
                                r.getActionId(),
                                AGUI_DELETE_MODE,
                                aguiCategory,
                                ainCategory,
                                distributionCompany
                        );
                    }

                } else {

                    String errorMessage = String.format("%d elements found in AIN for externalId: %s", externalData.size(), extIdOrg);
                    log.info(errorMessage);

                }
            }

        }else {
            String errorMessage = String.format("Could not build query for %s, %s", ainType.getCode(), aguiCategory);
            return new ResponseEntity<>("{ \"status\": \"kO\", \"error\": \"" + errorMessage + "\"}", HttpStatus.BAD_REQUEST);
        }

        return new ResponseEntity<>("{ \"status\": \"ok\"}", HttpStatus.OK);

    }

    private ResponseEntity<Object> startBatchAguiCreationOrUpdate(String country, String aguiCategory, String ainCategory, String type, String distributionCompany, String dateFrom, String dateTo,
                                                         Integer chunkSize, boolean forceExtraction, long skipRecords, long numberOfBlocks, String mode) throws IOException {
        log.info("Begin startBatchAgui for mode: " + mode);

        String category;

        Long jobId = 0L;

        category = aguiCategory + ainCategory;

        Parameters p = parametersRepository.findByCountryAndEnvironmentAndName(COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase(), "MAX_CHUNK");

        if (p != null && chunkSize <= Integer.parseInt(p.getValue())){

            if (validateDates(dateFrom, dateTo)) {

                Integer countInProgress = extractorService.getCountInProgressExecution(ExtractorConstants.STEP_AGUI);
                Boolean isLastExecution = extractorService.getExecutionLast24Hours(country, category, ExtractorConstants.AGUI_EQUIPMENT);
                Boolean isCurrentExecution = extractorService.getCurrentExecution(country, category, ExtractorConstants.AGUI_EQUIPMENT);
                if (countInProgress < defaultCountInProgress) {
                    if ((!isLastExecution && !isCurrentExecution) || forceExtraction) {

                        List<CategoriesMappingDTO> categories = rulesServices.getCategoriesMappingForCats(country, ainCategory, aguiCategory);

                        if (!categories.isEmpty()) {

                            final Optional<RulesDTO> opt = categories.get(0).getRules().stream().filter(r -> r.getEntityType().equalsIgnoreCase(type)).findFirst();
                            if (opt.isEmpty()) {
                                return new ResponseEntity<>("No rule found for: " + country + " , " + aguiCategory + " , " + ainCategory + " , " + type, HttpStatus.PRECONDITION_FAILED);
                            }
                            else {
                                if ((mode.equalsIgnoreCase(CREATION_MODE) && opt.get().isEnableCreation()) || mode.equalsIgnoreCase(UPDATE_MODE) ) {
                                    jobId = extractAgui(country, aguiCategory, ainCategory, type, distributionCompany, dateFrom, dateTo, chunkSize, skipRecords, numberOfBlocks, false, mode, null, null);
                                } else {
                                    ErrorResponseTO errorResponse = new ErrorResponseTO("Creation not allowed for the rule: " + country + " , " + aguiCategory + " , " + ainCategory + " , " + type);
                                    return new ResponseEntity<>(errorResponse, HttpStatus.PRECONDITION_FAILED);
                                }
                            }
                        }
                        else {
                            return new ResponseEntity<>("No rule found for: " + country + " , " + aguiCategory + " , " + ainCategory + " , " + type, HttpStatus.PRECONDITION_FAILED);
                        }

                    } else {
                        log.info("Nothing to extract for : " + category + " in: " + country);
                    }
                } else {
                    log.error(msgError.replace("$", String.valueOf(defaultCountInProgress)));
                    throw new TooManyRequestException(msgError.replace("$", String.valueOf(defaultCountInProgress)));
                }
            } else {
                return new ResponseEntity<>("Dates must be in format YYYY-MM-DD HH24:MI:SS", HttpStatus.PRECONDITION_FAILED);

            }
        }else{
            if (p != null) {
                return new ResponseEntity<>("Maximum value for chunkSize parameter: " + p.getValue(), HttpStatus.PRECONDITION_FAILED);
            }

            else{
                return new ResponseEntity<>("MAX_CHUNK parameter not found in the rules db", HttpStatus.PRECONDITION_FAILED);
            }

        }

        log.info("End startBatchAgui for mode: " + mode);
        return new ResponseEntity<>(String.format("{ \"status\": \"ok\", \"job_execution_id\" : %.0f }", jobId.floatValue()), HttpStatus.OK);


    }



    private boolean validateDates(String dateFrom, String dateTo){

        boolean result = true;
        Date d;
        DateTimeFormatter formatter = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss");

        try {
            if (StringUtils.isNotBlank(dateFrom)) {
                LocalDateTime.parse(dateFrom, formatter);
            }
            if (StringUtils.isNotBlank(dateTo)) {
                LocalDateTime.parse(dateTo, formatter);
            }

        } catch (DateTimeParseException e) {
            result = false;
        }

        return result;

    }

    private Long extractAgui(String country, String aguiCategory,  String ainCategory, String type, String distributionCompany, String dateFrom,
                             String dateTo, Integer chunkSize, long skipRecords, long numberOfBlocks, boolean skipXML, String mode, String matricolas,
                             String actionType) {

        String category;
        category = aguiCategory + ainCategory;

        if (StringUtils.isBlank(matricolas))
            matricolas = "";

        JobParameters jobParameters = new JobParametersBuilder()
                .addString("aguiCategory", aguiCategory)
                .addString("ainCategory", ainCategory)
                .addString("type", type)
                .addString("distributionCompany", distributionCompany != null ? distributionCompany : "")
                .addString("dateFrom", dateFrom != null ? dateFrom : "")
                .addString("dateTo", dateTo != null ? dateTo : "")
                .addDouble("chunkSize", chunkSize != null ? chunkSize.doubleValue() : 2000)
                .addString("country", country)
                .addString("typologyTemplate", ExtractorConstants.AGUI_EQUIPMENT)
                .addLong("skipRecords", skipRecords)
                .addLong("numberOfBlocks", numberOfBlocks)
                .addString("skipXML", Boolean.toString(skipXML))
                .addString("mode", mode)
                .addString("matricolas", matricolas)
                .addString("actionType", actionType != null ? actionType : "")
                .addLong("startAt", System.currentTimeMillis()).toJobParameters();
        try {
            JobExecution je;

            switch(AINTypesEnum.valueOf(type)) {
                case CONDUCTOR_TYPE:
                    je = extractorJobLauncher.run(jobConductorType, jobParameters);
                    return je.getId();
                case TRANSFORMER_TYPE:
                    je = extractorJobLauncher.run(jobTransformerType, jobParameters);
                    return je.getId();
                case GROUND_DEVICE_TYPE:
                    je = extractorJobLauncher.run(jobGrDeviceType, jobParameters);
                    return je.getId();
                default:
                    je = extractorJobLauncher.run(jobAgui, jobParameters);
                    return je.getId();
            }

        } catch (JobExecutionAlreadyRunningException | JobRestartException | JobInstanceAlreadyCompleteException |
                 JobParametersInvalidException e) {
            log.error("Error during the job's execution: {}", e.getMessage());
            throw new RuntimeException(e);
        }
    }

    private Long extractDBRules(String country, String ainCategory, String type, String distributionCompany, String actionType) {

        JobParameters jobParameters = new JobParametersBuilder()
                .addString("ainCategory", ainCategory)
                .addString("type", type)
                .addString("distributionCompany", distributionCompany)
                .addString("country", country)
                .addString("actionType", actionType)
                .addLong("run.id", System.currentTimeMillis())
                .toJobParameters();
        try {
            JobExecution je;

            switch(AINTypesEnum.valueOf(type)) {
                case CONDUCTOR_TYPE:
                    je = extractorJobLauncher.run(jobConductorTypeDBRules, jobParameters);
                    return je.getId();
                case TRANSFORMER_TYPE:
                    je = extractorJobLauncher.run(jobTransformerTypeDBRules, jobParameters);
                    return je.getId();
                case GROUND_DEVICE_TYPE:
                    je = extractorJobLauncher.run(jobGrDeviceTypeDBRules, jobParameters);
                    return je.getId();
                default:
                    throw new ValueNotAllowedException(type, new String[] {"CONDUCTOR_TYPE", "TRANSFORMER_TYPE", "GROUND_DEVICE_TYPE"});
            }

        } catch (JobExecutionAlreadyRunningException | JobRestartException | JobInstanceAlreadyCompleteException |
                 JobParametersInvalidException e) {
            log.error("Error during the job's execution: {}", e.getMessage());
            throw new RuntimeException(e);
        }
    }

    private Long extractAguiDocuments(String type, String country, String aguiCategory, String ainCategory,
                                      String distributionCompany, String dateFrom, String dateTo, Integer chunkSize, String actionType,
                                      long skipRecords, long numberOfBlocks, String mode, String matricolas) {

        String category;
        category = aguiCategory + ainCategory;

        if (StringUtils.isBlank(matricolas))
            matricolas = "";

        JobParametersBuilder jobParameters = new JobParametersBuilder()
                .addString("aguiCategory", aguiCategory)
                .addString("ainCategory", ainCategory)
                .addString("type", type)
                .addString("distributionCompany", distributionCompany != null ? distributionCompany : "")
                .addString("dateFrom", dateFrom != null ? dateFrom : "")
                .addString("dateTo", dateTo != null ? dateTo : "")
                .addDouble("chunkSize", chunkSize != null ? chunkSize.doubleValue() : 2000)
                .addString("country", country)
                .addString("typologyTemplate", ExtractorConstants.AGUI_EQUIPMENT)
                .addLong("skipRecords", skipRecords)
                .addLong("numberOfBlocks", numberOfBlocks)
                .addString("mode", mode)
                .addString("matricolas", matricolas)
                .addLong("startAt", System.currentTimeMillis());
        if (StringUtils.isEmpty(actionType)) {
            jobParameters.addString("actionType", "C");
        } else {
            jobParameters.addString("actionType", actionType);
        }
        try {
            JobExecution je = extractorJobLauncher.run(jobAuiDocument, jobParameters.toJobParameters());
            return je.getId();
        } catch (JobExecutionAlreadyRunningException | JobRestartException | JobInstanceAlreadyCompleteException |
                 JobParametersInvalidException e) {
            log.error("Error during the job's execution: {}", e.getMessage());
            throw new RuntimeException(e);
        }
    }

    @Override
    @DeleteMapping("/truncate")
    public void deleteData() {
        log.info("Start deleteData");
        extractorService.deleteData();
        extractorService.updateTruncate();
        log.info("End deleteData");
    }

    @Override
    @GetMapping("/agui/getjobs")
    public List<JobExecutionInfo> getAguiJobs(){

        // TODO: Check if necessary
        return null;
        /*

        log.info("Begin getAguiJobs in controller");
        ArrayList<JobExecutionInfo> results = new ArrayList<>();
        Set<JobExecution> jobs = extractorJobExplorer.findRunningJobExecutions("jobAgui");
        for(JobExecution jobExecution : jobs){
            JobExecutionInfo jobExecutionInfo = new JobExecutionInfo();
            jobExecutionInfo.setId(jobExecution.getId());
            jobExecutionInfo.setName(jobExecutionInfo.getName());
            jobExecutionInfo.setStatus(jobExecution.getStatus().name());
            results.add(jobExecutionInfo);
        }
        log.info("End getAguiJobs in controller");
        return results;


         */
    }

    @Override
    @GetMapping("/ain/getjobs")
    public ArrayList<JobExecutionInfo> getAinJobs(){

        // TODO: Check if necessary
        return null;
        /*
        log.info("Begin getAinJobs in controller");
        ArrayList<JobExecutionInfo> results = new ArrayList<>();
        Set<JobExecution> jobs = extractorJobExplorer.findRunningJobExecutions("jobAin");
        for(JobExecution jobExecution : jobs){
            JobExecutionInfo jobExecutionInfo = new JobExecutionInfo();
            jobExecutionInfo.setId(jobExecution.getJobId());
            jobExecutionInfo.setName(jobExecutionInfo.getName());
            jobExecutionInfo.setStatus(jobExecution.getStatus().name());
            results.add(jobExecutionInfo);
        }
        log.info("End getAinJobs in controller");
        return results;

         */
    }

    @Override
    @GetMapping("/lastExecution")
    public ResponseEntity<String> getLastExecution(String category, String country) {
        log.info("Begin getLastExecution in controller");
        String result = extractorService.getLastExecution(country, category);
        log.info("End getLastExecution in controller");
        return new ResponseEntity<>(result, HttpStatus.OK);
    }

    @Override
    @GetMapping(value = "/ain/retryRequest", produces = MediaType.APPLICATION_JSON_VALUE)
    public String retryRequest(String country, String force,String requestId, String id) {

        Optional<LogDTO> logOpt;
        LogDTO logInfo;
        String result;
        String newRequestId;
        boolean forceRetry = false;



        if (StringUtils.isNotBlank(requestId))
            logOpt = logService.getLogByRequestIdCountry(requestId, country);
        else if (StringUtils.isNotBlank(id)) {
            try {
                logOpt = logService.getLogById(Integer.parseInt(id));
            } catch (Exception e) {
                logOpt = Optional.empty();
            }
        }else
            logOpt = Optional.empty();

        if (logOpt.isPresent()) {

            logInfo = logOpt.get();

            if (StringUtils.isNotBlank(force) && force.equalsIgnoreCase("true"))
                forceRetry  = true;
            result = processRetry(logInfo, country, forceRetry);

        }else{
            result = "{\"status\": \"ko\", \"error\": \"No log found for country: " + country + " requet_id: "  + requestId  + " id: " + id + "\"}";
        }

        return result;

    }

    @Override
    @GetMapping(value = "/ain/retryAllFailedRequests", produces = MediaType.APPLICATION_JSON_VALUE)
    public String retryAllFailedRequests(String country, String force, String jobExecutionId){

        List<LogDTO> logs = new ArrayList<>();
        List<LogDTO> logsFailed;
        List<LogDTO> logsError;
        List<LogDTO> logsNull;
        String result;
        String newRequestId;
        String oldRequestId;
        int job;
        JsonArray results = new JsonArray();
        boolean forceRetry = false;

        if (StringUtils.isNotBlank(jobExecutionId)){

            try {
                job = Integer.parseInt(jobExecutionId);
                logsFailed = logService.getLogsByJobExecutionIdCountryStatus(job, country, "FAILED");
                logsError = logService.getLogsByJobExecutionIdCountryStatus(job, country, "ERROR");
                logsNull = logService.getLogsByJobExecutionIdCountryStatus(job, country, null);

                if (CollectionUtils.isNotEmpty(logsFailed))
                        logs.addAll(logsFailed);

                if (CollectionUtils.isNotEmpty(logsError))
                    logs.addAll(logsError);

                if (CollectionUtils.isNotEmpty(logsNull))
                    logs.addAll(logsNull);


            }catch(Exception e){
                result = "{\"status\": \"ko\", \"error\": \" jobExcecutionId: " + jobExecutionId + " must be a number\"}";
                logs = null;
            }
        }else {
            result = "{\"status\": \"ko\", \"error\": \" jobExcecutionId: " + jobExecutionId + " must be a number\"}";
            results.add(new JsonParser().parse(result).getAsJsonObject());
            logs = null;
        }

        if (CollectionUtils.isNotEmpty(logs)) {

            if (StringUtils.isNotBlank(force) && force.equalsIgnoreCase("true"))
                forceRetry  = true;

            for (LogDTO logInfo : logs) {

                result = processRetry(logInfo, country, forceRetry);
                if (StringUtils.isNotBlank(result))
                    results.add(result);


            }
        }else{
            result = "{\"status\": \"ko\", \"error\": \"No log found for country: " + country + " jobExecutionId: " + jobExecutionId + "\"}";
            results.add(new JsonParser().parse(result).getAsJsonObject());
        }

        return results.toString();


    }

    private String processRetry(LogDTO logInfo, String country, boolean force){

        String result = null;

        if (force || (StringUtils.isBlank(logInfo.getMigrationStatus()) || (StringUtils.isNotBlank(logInfo.getMigrationStatus()) &&
                (logInfo.getMigrationStatus().equalsIgnoreCase("FAILED") || logInfo.getMigrationStatus().equalsIgnoreCase("ERROR"))) ||
                (StringUtils.isNotBlank(logInfo.getRequestId()) && logInfo.getRequestId().toUpperCase().startsWith("ERROR")))) {

            // Set the requet to READY to try it again
            logInfo.setMigrationStatus("READY");
            logService.updateLog(logInfo);

            result = "{\"status\": \"ok\", \"request_id\": \"" + logInfo.getRequestId() + "\", \"status\": \"" + logInfo.getMigrationStatus() + "\"}";

        }

        return result;

    }

    @Override
    @GetMapping(value = "/agui/stopJob", produces = MediaType.APPLICATION_JSON_VALUE)
    public ResponseEntity<String> stopJob(String jobId){
        log.info("Stopping Job:" + jobId);

        boolean stopping = false;
        try {
            jobService.stopJob(Long.parseLong(jobId));
            stopping = true;
        } catch (JobExecutionNotRunningException e) {
            String errorMessage = String.format("No job running with id %s", jobId);
            return new ResponseEntity<>("{ \"status\": \"kO\", \"error\": \"" + errorMessage + "\"}", HttpStatus.NOT_MODIFIED);
        } catch (NoSuchJobExecutionException e) {
            String errorMessage = String.format("No job found with id %s", jobId);
            return new ResponseEntity<>("{ \"status\": \"kO\", \"error\": \"" + errorMessage + "\"}", HttpStatus.NO_CONTENT);
        } catch (Exception e) {
            String errorMessage = String.format("Unkown exception stopping the job  with id %s", jobId);
            e.printStackTrace();
            return new ResponseEntity<>("{ \"status\": \"kO\", \"error\": \"" + errorMessage + "\"}", HttpStatus.NO_CONTENT);
        }
        String result = String.format("{\"id\": \" + %s + \", \"status\": \" %s \"}", jobId, stopping?"stopping":"couldn't be stopped");
        return new ResponseEntity<>(result, HttpStatus.OK);

    }



    @Override
    @GetMapping(value = "/resetErrorAmsRules", produces = MediaType.APPLICATION_JSON_VALUE)
    public ResponseEntity<String> resetErrorAmsRules(String country, AINTypesEnum type, String aguiCategory, String ainCategory) {
        final List<CategoriesMappingDTO> categories = rulesServices.getCategoriesMappingForCats(country, aguiCategory, ainCategory);
        final List<CategoriesMappingDTO> catsWithRules = new ArrayList<>();
        if (categories != null && !categories.isEmpty()) {
            for (CategoriesMappingDTO cat: categories) {
                Set<RulesDTO> fiteredCatRules = rulesServices.getRulesByCategory(mapper.convertValue(cat, CategoriesMapping.class)).stream()
                        .filter(r -> {
                            if (type != null && !r.getEntityType().equalsIgnoreCase(type.getCode()))
                                return false;
                            String currentStatus = r.getCurrentExecutionStatus();
                            if (currentStatus != null && currentStatus.equalsIgnoreCase("ERROR-AMS"))
                                return true;
                            else
                                return false;
                        }).map(entity -> mapper.convertValue(entity, RulesDTO.class)).collect(Collectors.toSet());
                if (!fiteredCatRules.isEmpty()) {
                    cat.setRules(fiteredCatRules);
                    catsWithRules.add(cat);
                }
            }
            if (!catsWithRules.isEmpty()) {
                Map<Integer, Boolean> data = orchestrationService.resetErrorAmsRules(catsWithRules);
                return new ResponseEntity<>(new Gson().toJson(data), HttpStatus.OK);
            } else {
                String typeResp = type==null?"":" , " + type.getCode();
                return new ResponseEntity<>("No rule found for: " + country + " , " + aguiCategory + " , " + ainCategory + typeResp, HttpStatus.NOT_FOUND);
            }
        } else {
            String typeResp = type==null?"":" , " + type.getCode();
            return new ResponseEntity<>("No rule found for: " + country + " , " + aguiCategory + " , " + ainCategory + typeResp, HttpStatus.NOT_FOUND);
        }
    }

    /* UNCOMMENT TO WRITE LOG TRACES ON orquestration_log table

    private int getIdLogFortraces(){

        Optional<Parameters> p;

        if (this.idLogFortraces == null){

            p = rulesServices.getParameterByName(RulesServices.COUNTRY_GLOBAL, System.getenv("ENV").toUpperCase( ), "TEMP_ID_ORCHESTRATION_LOG");
            this.idLogFortraces =  p.map(value -> Integer.parseInt(value.getValue())).orElse(-1);

        }

        return this.idLogFortraces;

    } */

    private String  getTypeForSemaphore(String type){

        String finalType= "";
        AINTypesEnum ainType;

        ainType = AINTypesEnum.valueOf(type);

        switch(ainType) {

            case GLOBAL_TYPE:
                finalType = "GT";
                break;

            case SUBTYPE:
                finalType = "ST";
                break;

            case LOCAL_TYPE:
            case SUBCOMP_TYPE:
                finalType = "LT";
                break;

            case OEM_MODEL:
            case SUBCOMP_DSHEET:
                finalType = "OM";
                break;

            case EQUIPMENT:
            case SUBEQ:
                finalType = "EQ";
                break;
        }

        return finalType;

    }

    private boolean setDocsRuleInProgress(Rules rule, String aguiCategory, String ainCategory){

        CategoriesRunning cr;
        boolean result = false;
        String country;

        country = rule.getCategoriesMapping().getCountry();

        boolean emptyAinToAguiFlow = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow(country, aguiCategory, ainCategory, "AIN-TO-AGUI").isEmpty();
        boolean emptyAguiToAinFlow = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow(country, aguiCategory, ainCategory, "AGUI-TO-AIN").isEmpty();
        if (emptyAinToAguiFlow && emptyAguiToAinFlow) {

            // Insert a record in the Semaphore
            cr = new CategoriesRunning();
            cr.setFlow("AGUI-TO-AIN");
            cr.setAguiCategory(aguiCategory);
            cr.setAinCategory(ainCategory);
            cr.setCountry(country);
            try {
                lockDbService.addNewRunningCategory(cr);
                result = true;
                orchestrationService.updateRuleDataForDocs(rule, null, rule.getEntityType());

            } catch (Exception e) {
                log.error("INSERT-SEMAPHORE-ERROR: " + e.getMessage());
                result = false;
            }
        }

        return result;
    }


    private void restoreAbbandonedInProgressRules(String entityType, Parameters p){

        List<Rules> rulesInProgress;
        int maxHoursInProgrress = 24;
        Map<Integer, Boolean> result = new HashMap<>();


        if (p != null)
            maxHoursInProgrress = Integer.parseInt(p.getValue());
        else
            log.error("Missing parameter MAX_HOURS_IN_PROGRESS. Using default value: 24)");

        rulesInProgress = rulesServices.getRulesByStatusAndEntityType("IN_PROGRESS", entityType);

        for (Rules r : rulesInProgress){

            CategoriesMapping catMap;
            catMap = r.getCategoriesMapping();
            CategoriesMappingDTO cat = new CategoriesMappingDTO();
            cat.setCountry(catMap.getCountry());
            cat.setAguiCategory(catMap.getAguiCategory());
            cat.setAinCategory(catMap.getAinCategory());

            // If the rule is IN_PROGRESS for more than 24 hours, or the rule is IN_PROGRESS but
            // don't have current_execution_date, we restore the rule to READY
            if (r.getCurrentExecutionDate() != null) {
                long hours = (new Date().getTime() - (r.getCurrentExecutionDate().getTime())) / MILLI_TO_HOUR;

                if (hours >= maxHoursInProgrress) {

                    orchestrationService.actionsToResetRule(cat, mapper.convertValue(r, RulesDTO.class), result);

                }
            }else{
                orchestrationService.actionsToResetRule(cat, mapper.convertValue(r, RulesDTO.class), result);
            }

        }

    }

    private String checkIfAutoRequestToEnqueueIsRunningWithSemaphore() throws ParseException {

        String previousType;
        String result = "";
        List<CategoriesRunning> cr;
        String typeForSemaphore = "AE";

        // Check if the AutoRequestToEnqueue is currently blocked in the semaphore
        cr = lockDbService.getByCountryAndAguiCategoryAndAinCategoryAndFlow("ALL", typeForSemaphore, typeForSemaphore, "ORCHESTRATOR");

        // If we have a record in the semaphore, the category is currently running, we skip it.
        if (cr.size() == 1) {

            result = "Execution skipped, the autoRequestToEnqueue is currently runing...";
        }

        return result;

    }


    private synchronized boolean blockAutoRequesToEnqueueOnSemaphore() {


        // Block the execution for AutoRequestToEnqueue on the semaphore
        // Insert a record in the Semaphore
        boolean result = true;
        String typeForSemaphore = "AE";

        CategoriesRunning cr = new CategoriesRunning();
        cr.setFlow("ORCHESTRATOR");
        cr.setAguiCategory(typeForSemaphore);
        cr.setAinCategory(typeForSemaphore);
        cr.setCountry("ALL");
        try {
            lockDbService.addNewRunningCategory(cr);
        } catch (Exception e) {
            log.error("INSERT-SEMAPHORE-ERROR: " + e.getMessage());
            result = false;
        }

        return result;

    }

}
